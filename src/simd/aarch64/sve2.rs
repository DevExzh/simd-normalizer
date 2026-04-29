//! SVE2 scanner backend.
//!
//! Mirrors the role of [`super::neon`]: produces a 64-bit bitmask where set
//! bits flag byte positions in a 64-byte chunk whose value is `>= bound`.
//!
//! ## Implementation notes
//!
//! AArch64 SVE/SVE2 intrinsics are **not** exposed by stable Rust's
//! `core::arch::aarch64` module at the project's MSRV (1.91). Concretely, the
//! types `svuint8_t` / `svbool_t` and the helper functions `svld1_u8`,
//! `svdup_n_u8`, `svcmpge_u8`, `svptrue_b8`, `svdup_u8_z`, `svmaxv_u8` are
//! unavailable on stable. They live behind nightly's `stdarch_aarch64_sve`
//! feature gate.
//!
//! Because SVE register types cannot be carried across function boundaries
//! on stable, the six logical helpers from `impl_scanner!` (`simd_load`,
//! `simd_splat`, `simd_cmpge_mask`, `simd_any_ge`, plus the macro-generated
//! `scan_chunk` / `scan_and_prefetch`) are realised here as a single
//! `#[target_feature(enable = "sve2")]` `unsafe fn` per logical step,
//! implemented with `core::arch::asm!` (`nostack, preserves_flags,
//! readonly`), mirroring the discipline used in [`crate::simd::prefetch`].
//! Where helpers must hand SVE state between steps, we keep the SVE work
//! within a single asm block and exchange only scalar (`u32` / `u64` /
//! `*const u8`) values across function boundaries — i.e. we never construct a
//! `SimdVec` value visible to safe Rust.
//!
//! Once SVE2 intrinsics are stabilised (or the project bumps MSRV onto a
//! toolchain that ships them), this file should migrate to the intrinsic
//! form sketched in `docs/superpowers/specs/2026-04-28-aarch64-optimization-design.md`
//! (Components → B). The asm here was written to follow that intrinsic
//! recipe step-for-step so the migration is mechanical.
//!
//! ## Vector length
//!
//! The architecture allows VL ∈ {128, 256, 384, 512, 640, 768, 896, 1024,
//! 1152, 1280, 1408, 1536, 1664, 1792, 1920, 2048} bits (multiples of 128,
//! up to 2048). Current shipping cores (Neoverse-N2/V2/V3, Ampere AmpereOne)
//! implement VL = 128 bits.
//!
//! To keep the implementation correct and predictable on **any** VL we run
//! the scan in 16-byte (128-bit) sub-vectors using `whilelo`-bounded
//! predicates. The asm only touches the low 128 bits of `z0..z5` (which
//! alias the NEON `v0..v5` registers), so the wider lanes on a 256+-bit
//! core are simply unused. This sacrifices peak throughput on >128-bit VLs
//! but matches NEON's chunk shape exactly — `LANES = 16`,
//! `VECS_PER_CHUNK = 4` — so the macro contract is preserved.
//!
//! Picking up wider VLs is left as a follow-up (the spec calls this out as
//! "iterate on perf" in §B); see `simd_cmpge_mask_sve_full_vl` in the
//! TODO at the bottom of this file.
//!
//! ## Macro choice
//!
//! Spec option (a): write a parallel scanner generator for SVE2 instead of
//! reusing `impl_scanner!`. This keeps the NEON / x86 / wasm backends
//! untouched. Because we can't carry an `svuint8_t` across function
//! boundaries on stable, even option (b) (a trait-driven generalisation of
//! the existing macro) wouldn't help — the public-helper layer is what
//! breaks. We therefore inline the 4 sub-vector loop directly in
//! `scan_chunk` below rather than introducing a single-use macro.
//!
//! ## Predicate-to-bitmask packing
//!
//! For each 16-byte sub-vector we emit, in one `asm!` block:
//!
//! 1. `whilelo p0.b, xzr, #16`          — predicate covers low 16 lanes.
//! 2. `mov     z1.b, {bound:w}`          — `svdup_n_u8(bound)` equivalent.
//! 3. `ld1b    {z0.b}, p0/z, [ptr]`      — `svld1_u8` equivalent.
//! 4. `cmphs   p1.b, p0/z, z0.b, z1.b`   — `svcmpge_u8` equivalent.
//! 5. `mov     z2.b, p1/z, #-1`          — `svdup_u8_z(p1, 0xFF)` equivalent.
//! 6. NEON-style movemask on `v2.16b`:
//!    - `ldr q3, [bit_mask]`              — load `[1,2,4,...,1,2,4,...]`.
//!    - `and v2.16b, v2.16b, v3.16b`      — mask each lane to its bit value.
//!    - `addv b4, v2.8b` then `umov`      — low-half byte = bits 0..7.
//!    - `ext  v5.16b, v2.16b, v2.16b, #8` — high-half byte.
//!    - `addv b5, v5.8b` then `umov`      — high-half byte = bits 8..15.
//!    - `orr  result, lo, hi, lsl #8`.
//!
//! The output `u32`'s **bit `n` corresponds to byte position `n` within the
//! 16-byte sub-vector** (LSB = first byte). `scan_chunk` shifts each
//! sub-vector mask by `i * 16` to compose the 64-bit chunk mask, so chunk
//! bit `n` = byte position `n` within the 64-byte chunk (LSB = first byte).
//! This matches NEON's contract exactly.
//!
//! ## Early-out
//!
//! `simd_any_ge_sve` is a separate function executing `whilelo + ld1b +
//! cmphs + ptest` and returning a `bool`. Predicate-only path: we never
//! materialise the 0xFF/0x00 byte vector when only a presence check is
//! needed, matching the spec's optional fast path. (The `cmphs` then
//! `b.any` form: SVE's `ptest` writes NZCV; we read it back via `cset`.)

#![cfg(target_arch = "aarch64")]

/// Number of bytes per logical sub-vector. Pinned to 16 (NEON-equivalent)
/// regardless of runtime SVE VL — see "Vector length" in the module
/// docstring.
const SUB_VEC_BYTES: usize = 16;

/// Number of 16-byte sub-vectors per 64-byte chunk.
const SUB_VECS_PER_CHUNK: usize = 64 / SUB_VEC_BYTES;

/// Scan one 16-byte sub-vector; return a 16-bit bitmask of `byte >= bound`.
///
/// Logical equivalent of (NEON's) `simd_load + simd_splat + simd_cmpge_mask`,
/// but emitted as a single `asm!` block because SVE register state can't
/// safely cross stable-Rust function boundaries (`svuint8_t` is not yet
/// exported from `core::arch::aarch64`). Steps inside the block are
/// commented to make the migration to intrinsics mechanical when they
/// stabilise.
///
/// Bit `n` of the returned `u32` corresponds to byte offset `n` within the
/// 16 bytes starting at `ptr`. Bits 16..31 are zero.
///
/// # Safety
/// - `ptr` must be valid for at least 16 bytes of read access.
/// - SVE2 must be available at runtime (`is_aarch64_feature_detected!`).
#[target_feature(enable = "sve2")]
#[inline]
unsafe fn simd_cmpge_mask_sve(ptr: *const u8, bound: u8) -> u32 {
    let lo: u64;
    let hi: u64;
    unsafe {
        core::arch::asm!(
            // Step 1: predicate covers the low 16 lanes only.
            // svwhilelt_b8(0, 16) ≡ `whilelo p0.b, xzr, #16`.
            "whilelo p0.b, xzr, {sixteen:x}",
            // Step 2: svdup_n_u8(bound) → z1.b.
            "mov     z1.b, {bound:w}",
            // Step 3: svld1_u8(p0, ptr) → z0.b.
            "ld1b    {{z0.b}}, p0/z, [{ptr}]",
            // Step 4: svcmpge_u8(p0, z0, z1) → p1.
            //   SVE compare-higher-or-same (unsigned).
            "cmphs   p1.b, p0/z, z0.b, z1.b",
            // Step 5: svdup_u8_z(p1, 0xFF) → z2.b (each lane: 0xFF or 0x00).
            "mov     z2.b, p1/z, #-1",
            // Step 6: NEON-style movemask on the low 128 bits (v2.16b).
            //   The Z[n] register aliases V[n] in its low 128 bits.
            "ldr     q3, [{bm_ptr}]",
            "and     v2.16b, v2.16b, v3.16b",
            "addv    b4, v2.8b",
            "umov    {lo:w}, v4.b[0]",
            "ext     v5.16b, v2.16b, v2.16b, #8",
            "addv    b5, v5.8b",
            "umov    {hi:w}, v5.b[0]",
            ptr     = in(reg)  ptr,
            bound   = in(reg)  bound,
            bm_ptr  = in(reg)  super::MOVEMASK_BIT_MASK.as_ptr(),
            sixteen = in(reg)  SUB_VEC_BYTES as u64,
            lo      = lateout(reg) lo,
            hi      = lateout(reg) hi,
            // Clobbered SIMD/predicate registers. We list both the Z and V
            // forms where they alias, mirroring the conservative discipline
            // used elsewhere in this crate.
            out("z0") _, out("z1") _, out("z2") _,
            out("v3") _, out("v4") _, out("v5") _,
            out("p0") _, out("p1") _,
            options(nostack, preserves_flags, readonly),
        );
    }
    ((lo as u32) & 0xFF) | (((hi as u32) & 0xFF) << 8)
}

/// Returns `true` iff any of the 16 bytes starting at `ptr` is `>= bound`.
///
/// Predicate-only fast path used by `scan_chunk` for the empty-sub-vector
/// early-out. Avoids the `cpy z, p/z, #-1` materialisation and the entire
/// NEON-style movemask reduction.
///
/// # Safety
/// Same as [`simd_cmpge_mask_sve`].
#[target_feature(enable = "sve2")]
#[inline]
unsafe fn simd_any_ge_sve(ptr: *const u8, bound: u8) -> bool {
    let any: u64;
    unsafe {
        core::arch::asm!(
            "whilelo p0.b, xzr, {sixteen:x}",
            "mov     z1.b, {bound:w}",
            "ld1b    {{z0.b}}, p0/z, [{ptr}]",
            "cmphs   p1.b, p0/z, z0.b, z1.b",
            // ptest sets NZCV; Z=1 iff p1 is all-zero under p0. The `any`
            // condition (svptest_any) is `!Z`, exposed via `cset NE`.
            "ptest   p0, p1.b",
            "cset    {any:x}, ne",
            ptr     = in(reg)  ptr,
            bound   = in(reg)  bound,
            sixteen = in(reg)  SUB_VEC_BYTES as u64,
            any     = lateout(reg) any,
            out("z0") _, out("z1") _,
            out("p0") _, out("p1") _,
            // Not preserves_flags: ptest writes NZCV.
            options(nostack, readonly),
        );
    }
    any != 0
}

/// Scan a 64-byte chunk. Returns a `u64` bitmask where set bits indicate
/// byte positions with values `>= bound`.
///
/// Mirrors the macro-generated NEON `scan_chunk` exactly (4 sub-vectors of
/// 16 bytes, with the `simd_any_ge` early-out per sub-vector).
///
/// # Safety
/// - `ptr` must be valid for 64 bytes of read access.
/// - SVE2 must be available at runtime.
#[target_feature(enable = "sve2")]
#[inline]
pub(crate) unsafe fn scan_chunk(ptr: *const u8, bound: u8) -> u64 {
    let mut mask: u64 = 0;
    let mut i = 0;
    while i < SUB_VECS_PER_CHUNK {
        // SAFETY: caller guarantees 64 readable bytes from `ptr`, so each
        // 16-byte sub-vector at `ptr + i*16` is in-bounds.
        let p = unsafe { ptr.add(i * SUB_VEC_BYTES) };
        if unsafe { simd_any_ge_sve(p, bound) } {
            let sub_mask = unsafe { simd_cmpge_mask_sve(p, bound) } as u64;
            mask |= sub_mask << (i * SUB_VEC_BYTES);
        }
        i += 1;
    }
    mask
}

/// Scan a 64-byte chunk and issue prefetch instructions.
///
/// # Safety
/// Same as [`scan_chunk`], plus `prefetch_l1` and `prefetch_l2` must point
/// into (or one cache line past) a readable allocation.
#[allow(dead_code)]
#[target_feature(enable = "sve2")]
#[inline(never)]
pub(crate) unsafe fn scan_and_prefetch(
    ptr: *const u8,
    prefetch_l1: *const u8,
    prefetch_l2: *const u8,
    bound: u8,
) -> u64 {
    use crate::simd::prefetch::{prefetch_l1_stream, prefetch_l2_stream};
    unsafe {
        prefetch_l1_stream(prefetch_l1);
        prefetch_l2_stream(prefetch_l2);
        scan_chunk(ptr, bound)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

// Compile-only smoke test: gated on `target_arch = "aarch64"` only (no
// `target_feature` check), so even on Apple Silicon (no SVE2) the build
// verifies that `scan_chunk` and `scan_and_prefetch` *compile*. The
// references are never called — at runtime on a non-SVE2 host the asm
// would SIGILL, and these casts are evaluated as plain pointer values.
#[cfg(all(test, target_arch = "aarch64"))]
mod compile_smoke {
    #[test]
    fn sve2_symbols_compile() {
        let _: unsafe fn(*const u8, u8) -> u64 = super::scan_chunk;
        let _: unsafe fn(*const u8, *const u8, *const u8, u8) -> u64 = super::scan_and_prefetch;
    }
}

// Runtime tests: gated on `target_feature = "sve2"` so they only execute
// on a host whose toolchain has SVE2 enabled at compile time. Apple
// Silicon does not have SVE2, so the gate evaluates to false and these
// tests are silently skipped — that's expected.
#[cfg(all(test, target_arch = "aarch64", target_feature = "sve2"))]
mod tests {
    use super::*;

    #[test]
    fn sve2_scan_all_below() {
        let data = [0x41u8; 64];
        let mask = unsafe { scan_chunk(data.as_ptr(), 0xC0) };
        assert_eq!(mask, 0);
    }

    #[test]
    fn sve2_scan_all_above() {
        let data = [0xFFu8; 64];
        let mask = unsafe { scan_chunk(data.as_ptr(), 0xC0) };
        assert_eq!(mask, u64::MAX);
    }

    #[test]
    fn sve2_scan_at_bound() {
        let data = [0xC0u8; 64];
        let mask = unsafe { scan_chunk(data.as_ptr(), 0xC0) };
        assert_eq!(mask, u64::MAX);
    }

    #[test]
    fn sve2_scan_mixed() {
        let mut data = [0x41u8; 64];
        data[0] = 0xC0;
        data[15] = 0xC0;
        data[16] = 0xC0;
        data[63] = 0xFF;
        let mask = unsafe { scan_chunk(data.as_ptr(), 0xC0) };
        let expected = (1u64 << 0) | (1u64 << 15) | (1u64 << 16) | (1u64 << 63);
        assert_eq!(mask, expected);
    }

    #[test]
    fn sve2_scan_every_position() {
        for pos in 0..64 {
            let mut chunk = [0u8; 64];
            chunk[pos] = 0xC0;
            let mask = unsafe { scan_chunk(chunk.as_ptr(), 0xC0) };
            assert_eq!(mask, 1u64 << pos, "SVE2: Expected only bit {pos} set");
        }
    }

    #[test]
    fn sve2_scan_bound_zero() {
        let data = [0x00u8; 64];
        let mask = unsafe { scan_chunk(data.as_ptr(), 0x00) };
        assert_eq!(mask, u64::MAX);
    }

    #[test]
    fn sve2_scan_and_prefetch_matches() {
        let mut data = [0x30u8; 64];
        data[7] = 0xD0;
        data[31] = 0xE5;
        let dummy = data.as_ptr();
        let m1 = unsafe { scan_chunk(data.as_ptr(), 0xC0) };
        let m2 = unsafe { scan_and_prefetch(data.as_ptr(), dummy, dummy, 0xC0) };
        assert_eq!(m1, m2, "Prefetch variant must produce identical bitmask");
    }

    #[test]
    fn sve2_matches_scalar() {
        let mut chunk = [0u8; 64];
        for (i, byte) in chunk.iter_mut().enumerate() {
            *byte = (i as u8).wrapping_mul(7);
        }
        let sve_mask = unsafe { scan_chunk(chunk.as_ptr(), 0xC0) };
        let scalar_mask = unsafe { crate::simd::scalar::scan_chunk(chunk.as_ptr(), 0xC0) };
        assert_eq!(sve_mask, scalar_mask, "SVE2 must match scalar");
    }

    #[test]
    fn sve2_matches_neon() {
        let mut chunk = [0u8; 64];
        for (i, byte) in chunk.iter_mut().enumerate() {
            *byte = (i as u8).wrapping_mul(13).wrapping_add(0x80);
        }
        let sve_mask = unsafe { scan_chunk(chunk.as_ptr(), 0xC0) };
        let neon_mask = unsafe { crate::simd::aarch64::neon::scan_chunk(chunk.as_ptr(), 0xC0) };
        assert_eq!(sve_mask, neon_mask, "SVE2 and NEON must agree");
    }

    #[test]
    fn sve2_any_ge_helper() {
        unsafe {
            let zeros = [0x00u8; 16];
            let ones = [0xFFu8; 16];
            let bound = [0xC0u8; 16];
            assert!(!simd_any_ge_sve(zeros.as_ptr(), 0xC0));
            assert!(simd_any_ge_sve(ones.as_ptr(), 0xC0));
            assert!(simd_any_ge_sve(bound.as_ptr(), 0xC0));
        }
    }
}
