# Performance audit — simd-normalizer (2026-04-19)

## Summary

Top-ranked findings by expected impact:

1. **`matching.rs` runs the full NFKC/casefold/skeleton pipeline 2–4× over `&str` via fixed-point iteration** — high/L. Each iteration re-scans + re-allocates the entire string; `skeleton()` internally does its own fixed-point loop too, so total passes over `&str` are `O(iters × iters)`.
2. **Prefetch is read-only — `prefetch_write` is defined but never called** — medium/XS. Output buffer sees no software prefetch even though write-allocate stalls dominate the passthrough memcpy.
3. **NFC/NFKC passthrough path falls back to per-char `feed_entry` for the last ASCII byte of every passthrough run** — medium/S. Defeats bulk `push_str` for the final byte; a dedicated `last_passthrough_starter: Option<char>` would let the fast path copy the entire run.

## Findings

### 1. Matching pipeline: three separate full-string passes, iterated to fixed point

**Location:** `src/matching.rs:72-90`
**Expected impact:** high
**Effort:** L

**Evidence:**
```rust
// matching.rs:72-81
let mut current = one_pass(input, opts);
for _ in 0..3 {
    let next = one_pass(&current, opts);
    if next == current { return current; }
    current = next;
}
// matching.rs:84-90
fn one_pass(input: &str, opts: &MatchingOptions) -> String {
    let nfkc = crate::nfkc().normalize(input);
    let folded = casefold::casefold(&nfkc, opts.case_fold);
    let skel = confusable::skeleton(&folded);
    let final_folded = casefold::casefold(&skel, opts.case_fold);
    final_folded.into_owned()
}
```

Plus `skeleton()` itself loops up to 8 times over `&str` (`src/confusable.rs:53-64`):
```rust
for _ in 0..8 {
    let mut mapped = String::with_capacity(current.len());
    for ch in current.chars() { confusable_map_char(ch, &mut mapped); }
    let next = crate::nfd().normalize(&mapped).into_owned();
    if next == current { return next; }
    current = next;
}
```

**Recommendation:** Fuse the pipeline into a single producer/consumer state machine that operates on a stream of decomposed `(char, ccc)` entries: feed each code point through `compat_decompose → casefold_char → confusable_map_char → canonical_reorder → recompose`, accumulating into one output `String`. Today, for a typical ASCII input, the pipeline walks the string and allocates up to 4×(NFKC+casefold+NFD+map+NFD+casefold) = up to a dozen allocations and full walks. A single-pass fused variant plus a convergence check on a per-codepoint basis (rather than re-running the whole pipeline on the whole string) is the right model; reserve the re-entry fixed-point loop only for the rare codepoints whose mapping introduces new mappable codepoints.

### 2. `prefetch_write` is dead code — output buffer never prefetched

**Location:** `src/simd/prefetch.rs:51,98,134` (definitions), no call sites
**Expected impact:** medium
**Effort:** XS

**Evidence:**
```text
$ grep -rn 'prefetch_write' src/
src/simd/prefetch.rs:51:pub(crate) unsafe fn prefetch_write(ptr: *const u8) {
src/simd/prefetch.rs:98:pub(crate) unsafe fn prefetch_write(ptr: *const u8) {
src/simd/prefetch.rs:134:pub(crate) unsafe fn prefetch_write(_ptr: *const u8) {}
```

`normalizer.rs:478-484` already prefetches read-side L1/L2 for the scanner but nothing for the output buffer. The SIMD loop does `out.push_str(pass)` on every non-all-passthrough chunk; `String::push_str` path (via `Vec<u8>::extend_from_slice` → `memcpy`) has to wait for the write-allocate load on lines it hasn't touched.

**Recommendation:** In `normalize_impl`, once capacity is reserved, call `prefetch::prefetch_write(out.as_ptr().wrapping_add(out.len() + PREFETCH_L1_DISTANCE * CHUNK_SIZE))` once per SIMD iteration. A simple place is right after `simd::scan_and_prefetch` (line 483). This is ~2 LOC and is the canonical "why is the function even here if we don't call it" finding — it will produce measurable throughput gain on large-string normalization of mostly-ASCII input.

### 3. NFC/NFKC passthrough run handling splits off the last ASCII byte

**Location:** `src/normalizer.rs:571-584` (and `663-677` in the tail)
**Expected impact:** medium
**Effort:** S

**Evidence:**
```rust
if byte_pos > last_written {
    state.flush(&mut out, composes);
    let pass = &input[last_written..byte_pos];
    let n = pass.len();
    if composes {
        if n > 1 { out.push_str(&pass[..n - 1]); }
        let last_ch = pass.as_bytes()[n - 1] as char;
        state.feed_entry(last_ch, 0, &mut out, true);
    } else {
        out.push_str(pass);
    }
}
```

Every passthrough run in compose mode peels one byte off the end to feed `NormState` as a potential starter. For typical ASCII-dominated prose, that byte is often a space or letter that will never compose backward — meaning the per-run cost of a branch + `feed_entry` (which dispatches through composition/flush logic) is paid on every hop.

**Recommendation:** Keep the potential-starter as a 1-byte scalar shadow on `NormState` (or return early when CCC of the *next* codepoint is 0, since that will trigger a flush anyway and no composition is possible). Alternatively, if the following codepoint is a starter (CCC=0), skip the `feed_entry` splitting entirely and `push_str(pass)` the whole run, because starter-to-starter composition with an ASCII character is extremely rare (essentially only Hangul jamo, which can't be preceded by ASCII).

### 4. Quick-check table stores 2-bit data in full `u32` entries (4× the cache footprint)

**Location:** `src/tables/qc.rs:185` and all four `*_QC_TRIE_DATA` arrays
**Expected impact:** medium
**Effort:** M

**Evidence:**
```rust
// qc.rs:185
pub(crate) static NFC_QC_TRIE_DATA: &[u32] = &[
    0x00000000, 0x00000000, 0x00000000, 0x00000000, ...
    0x00000001, 0x00000001, 0x00000001, 0x00000001, 0x00000001, 0x00000000, ...
    0x00000002, 0x00000002, 0x00000001, 0x00000002, 0x00000002, 0x00000001, ...
```

Values are 0/1/2 — needs 2 bits, stored as 32-bit words. qc.rs alone is 2869 data rows × 8 entries × 4 bytes ≈ 90 KiB. Four QC tries exist (all `#[allow(dead_code)]` — see `mod.rs:102-150`; only the fused `ccc_qc` is used), but they still occupy static `.rodata`.

**Recommendation:** (a) Delete the four standalone QC tries (`lookup_nfc_qc`/`nfd_qc`/`nfkc_qc`/`nfkd_qc`) — `quick_check.rs` already uses the fused `lookup_ccc_qc` (lines 157, 217, 265), so these are truly dead. Saves ~280 KiB of static. (b) For `ccc_qc`, the data is 16-bit values (`(ccc<<8) | qc_bits`) packed into `u32`; change the generator to emit `&[u16]` and the `CodePointTrie::data` field width to 16 bits for this specific trie (or a `u16_data` variant). Halves the hot working set so the BMP stage3 fits comfortably in L1.

### 5. `CodePointTrie` stage3 data is u32-wide even when values fit in u8

**Location:** `src/tables/trie.rs:7-9`, `src/tables/ccc.rs`, `src/tables/ccc_qc.rs`
**Expected impact:** medium
**Effort:** M

**Evidence:**
```rust
// trie.rs:7-9
const BMP_SHIFT: u32 = 5;                   // 32-entry blocks
const BMP_MASK: u32 = (1 << BMP_SHIFT) - 1; // 0x1F
```
`ccc.rs`: `CCC_TRIE_DATA: &[u32]` with 825 rows × 8 entries = 6600 × 4 = 26.4 KiB. CCC only needs 1 byte (u8), so a u8-wide stage3 would fit in 6.6 KiB — well inside L1 even with qc_ccc also live.

**Recommendation:** Specialize `CodePointTrie` over the element type (or add parallel `data_u8` / `data_u16` fields) so CCC, casefold, and QC tries don't pay u32 storage for sub-byte data. CCC trie alone shrinks from ~26 KiB to ~6.6 KiB. Combined with finding #4 this keeps the entire hot BMP trie working set under 32 KiB L1D even when normalizer + qc + ccc + decomp are all resident.

### 6. CCC and decomposition use independent tries instead of sharing indices

**Location:** `src/tables/mod.rs:66-97` (separate `canonical_trie`, `compat_trie`, `ccc_trie`)
**Expected impact:** medium
**Effort:** M

**Evidence:**
```rust
// mod.rs:66-86 — separate tries
pub(crate) fn canonical_trie() -> CodePointTrie {
    CodePointTrie { bmp_index: decomposition::CANONICAL_BMP_INDEX,
                    data: decomposition::CANONICAL_TRIE_DATA, ... }
}
pub(crate) fn ccc_trie() -> CodePointTrie {
    CodePointTrie { bmp_index: ccc::CCC_BMP_INDEX, ... }
}
```

Decomposition trie payload *already encodes CCC* in bits 16-23 (see `mod.rs:30-32`), yet `ccc::lookup_ccc` and `decompose_from_trie_value` each do separate trie walks. The fused `ccc_qc_trie` exists and is used in `quick_check.rs`, but the normalizer hot path still calls `tables::lookup_ccc` on line 399 of `normalizer.rs` (inside `process_from_trie_nfd` fallback) and the decomposition trie on line 288.

**Recommendation:** Ensure `process_from_trie_nfd` never calls `tables::lookup_ccc` (line 399): the decomposition trie already has CCC for every codepoint. For singletons, store the target's CCC pre-baked into the singleton trie payload (widen `DECOMP_INFO_MASK` or repurpose upper bits to hold the *target's* CCC). Eliminates a second cache miss per decomposing singleton on the NFD fast-path.

### 7. `expansion_data` does an extra length-prefix indirection

**Location:** `src/tables/mod.rs:391-405` and `src/tables/decomposition.rs:181`
**Expected impact:** low
**Effort:** M

**Evidence:**
```rust
// mod.rs:395-404
let offset = (trie_value & DECOMP_INFO_MASK) as usize;
let table = match form { ... };
let length = table[offset] as usize;
Some(&table[offset + 1..offset + 1 + length])
```

The length is fetched from a cold separate `table[offset]` location before reading the payload. The hot 2-entry Latin case (used on line 357 of `normalizer.rs`) still pays for this length read even though it could be encoded in the trie value itself.

**Recommendation:** Inline the 1–2-code-point decompositions directly in the trie payload for BMP targets (most Latin/Greek/Cyrillic precomposed characters). Repurpose the currently-unused `BACKWARD_COMBINING`/`NON_ROUND_TRIP` bits or extend the trie value to 64 bits. Eliminates the `table[offset]` length miss for the common 2-entry case, which is >95% of decomposing Latin chars.

### 8. NEON movemask emulated with pairwise-add chain instead of `vshrn_n_u16` trick

**Location:** `src/simd/aarch64/neon.rs:60-74`
**Expected impact:** low
**Effort:** S

**Evidence:**
```rust
unsafe fn simd_cmpge_mask(a: SimdVec, b: SimdVec) -> u32 {
    let cmp = vcgeq_u8(a, b);
    let bit_mask = vld1q_u8(BIT_MASK.as_ptr());
    let masked = vandq_u8(cmp, bit_mask);
    let p1 = vpaddq_u8(masked, masked);
    let p2 = vpaddq_u8(p1, p1);
    let p3 = vpaddq_u8(p2, p2);
    let lo = vgetq_lane_u8(p3, 0) as u32;
    let hi = vgetq_lane_u8(p3, 1) as u32;
    lo | (hi << 8)
}
```

Three `vpaddq_u8` in serial = 3-cycle dependency chain, plus two `vget_lane` scalar extractions. On Apple M-series and Neoverse V1/V2, the well-known `vshrn_n_u16(..., 4)` trick (narrow + extract as u64) produces the 16-bit mask in 2 instructions and one cycle.

**Recommendation:** Replace with:
```rust
let cmp = vcgeq_u8(a, b);               // 0xFF / 0x00 per lane
let narrowed = vshrn_n_u16(vreinterpretq_u16_u8(cmp), 4); // u8x8 nibble mask
let bits = vget_lane_u64(vreinterpret_u64_u8(narrowed), 0);
```
This is a well-documented ~4× speedup for the NEON movemask emulation (see simdjson, sonic-rs writeups). Cuts one of the hottest instructions in the NEON scanner loop.

### 9. AVX-512 tail <64 bytes falls back to scalar instead of using masked load

**Location:** `src/normalizer.rs:596-685` (tail loop) vs AVX-512 capability in `src/simd/x86_64/avx512.rs`
**Expected impact:** low
**Effort:** M

**Evidence:** After the main SIMD loop ends at `pos + 64 > len`, `normalize_impl` enters a character-by-character scalar tail:
```rust
// normalizer.rs:598
let tail_has_work = bytes[pos..].iter().any(|&b| b >= bound);
if tail_has_work {
    let mut tail_pos = pos;
    while tail_pos < len { ... }
}
```
AVX-512BW provides `_mm512_mask_loadu_epi8` + `_mm512_mask_cmpge_epu8_mask`, which would let the final partial chunk produce a mask identical to a full chunk, re-using the set-bit walker (line 494) instead of a separate scalar control path.

**Recommendation:** Add a `scan_tail(ptr, bound, tail_len) -> u64` vtable entry for backends that can do masked loads (AVX-512 natively; SSE/AVX2/NEON can pad with a safe-to-read aligned overread or zero-fill into a stack buffer). This unifies the hot loop body for the last sub-chunk.

### 10. Set-bit walker already uses `trailing_zeros` + `& (x-1)` — good

**Location:** `src/normalizer.rs:493-496`
**Expected impact:** n/a (positive control)
**Effort:** n/a

**Evidence:**
```rust
while chunk_mask != 0 {
    let bit_pos = chunk_mask.trailing_zeros() as usize;
    chunk_mask &= chunk_mask.wrapping_sub(1);
```

Noted for completeness: the mask-consumer already uses the canonical `BLSR` / `TZCNT` pattern. The loop-carried dependency `mask → tzcnt → bit_pos → byte_pos → branches → next tzcnt` is unavoidable because the scalar work between bits is itself serial (NormState).

### 11. `normalize_to` copies through `Cow` instead of writing directly into `out`

**Location:** `src/normalizer.rs:770-775`, `804-809`, `838-843`, `872-877`
**Expected impact:** low
**Effort:** M

**Evidence:**
```rust
pub fn normalize_to(&self, input: &str, out: &mut String) -> bool {
    let result = normalize_impl(input, Form::Nfc);
    let already_normalized = matches!(&result, Cow::Borrowed(_));
    out.push_str(&result);
    already_normalized
}
```

`normalize_to` allocates a fresh `String` inside `normalize_impl`, then copies it into `out`. A true `normalize_to` API would pass `out` through to the SIMD loop and avoid both the inner `String::with_capacity` allocation and the final `push_str`.

**Recommendation:** Refactor `normalize_impl` to take `&mut String` output buffer and use it directly; the `Cow::Borrowed` short-circuit can be expressed as a bool return. This doubles the throughput of `normalize_to` on already-normalized ASCII and eliminates one allocation-per-call for mismatched inputs.
