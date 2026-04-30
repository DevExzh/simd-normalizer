//! Single-pass SIMD-guided normalizer implementations (NFC, NFD, NFKC, NFKD).
//!
//! The core loop scans 64-byte chunks via SIMD to identify passthrough regions
//! (all bytes below a form-dependent bound), copying them directly.  Non-passthrough
//! bytes trigger scalar decode + decompose + CCC sort + optional recomposition.

use alloc::borrow::Cow;
use alloc::string::String;

use crate::ccc::CccBuffer;
use crate::compose;
use crate::decompose::{self, DecompForm};
use crate::hangul;
use crate::quick_check;
use crate::simd;
use crate::simd::prefetch;
use crate::tables;
use crate::utf8;

// ---------------------------------------------------------------------------
// Form enum
// ---------------------------------------------------------------------------

/// Unicode normalization form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
    /// Canonical Decomposition, followed by Canonical Composition (NFC).
    Nfc,
    /// Canonical Decomposition (NFD).
    Nfd,
    /// Compatibility Decomposition, followed by Canonical Composition (NFKC).
    Nfkc,
    /// Compatibility Decomposition (NFKD).
    Nfkd,
}

impl Form {
    /// The SIMD passthrough byte bound for this form.
    ///
    /// Any byte below this value is guaranteed to not require normalization
    /// processing: it is either ASCII or a continuation byte of a character
    /// that does not need decomposition.
    ///
    /// - NFD/NFKD: 0xC0  (first byte of U+00C0, which decomposes)
    /// - NFC/NFKC: 0xC0  (same: characters >= U+00C0 may need processing)
    #[inline]
    fn passthrough_bound(self) -> u8 {
        match self {
            Form::Nfc | Form::Nfkc => 0xC0,
            Form::Nfd | Form::Nfkd => 0xC0,
        }
    }

    /// Whether this form applies canonical composition after decomposition.
    #[inline]
    fn composes(self) -> bool {
        matches!(self, Form::Nfc | Form::Nfkc)
    }

    /// Which decomposition form to use.
    #[inline]
    fn decomp_form(self) -> DecompForm {
        match self {
            Form::Nfc | Form::Nfd => DecompForm::Canonical,
            Form::Nfkc | Form::Nfkd => DecompForm::Compatible,
        }
    }

    /// Estimated output capacity for a given input length.
    #[inline]
    fn estimated_capacity(self, input_len: usize) -> usize {
        match self {
            Form::Nfc | Form::Nfkc => input_len,
            Form::Nfd | Form::Nfkd => input_len + input_len / 2,
        }
    }

    /// Run quick_check for this normalization form.
    #[inline]
    fn quick_check(self, input: &str) -> quick_check::IsNormalized {
        match self {
            Form::Nfc => quick_check::quick_check_nfc(input),
            Form::Nfd => quick_check::quick_check_nfd(input),
            Form::Nfkc => quick_check::quick_check_nfkc(input),
            Form::Nfkd => quick_check::quick_check_nfkd(input),
        }
    }
}

// ---------------------------------------------------------------------------
// NormState -- accumulation state for a starter + its combining marks
// ---------------------------------------------------------------------------

struct NormState {
    /// The current starter character (CCC == 0) being accumulated.
    current_starter: Option<char>,
    /// Combining marks (CCC > 0) following the current starter, not yet sorted.
    ccc_buf: CccBuffer,
}

impl NormState {
    #[inline]
    fn new() -> Self {
        NormState {
            current_starter: None,
            ccc_buf: CccBuffer::new(),
        }
    }

    /// Flush the current accumulation (starter + combining marks) to `out`.
    ///
    /// If `composes` is true, applies canonical composition.
    #[inline]
    fn flush(&mut self, out: &mut String, composes: bool) {
        let starter = match self.current_starter.take() {
            Some(s) => s,
            None => {
                // No starter -- flush any orphan combining marks (leading combiners).
                if !self.ccc_buf.is_empty() {
                    self.ccc_buf.sort_in_place();
                    for entry in self.ccc_buf.as_slice() {
                        out.push(entry.ch);
                    }
                    self.ccc_buf.clear();
                }
                return;
            },
        };

        if self.ccc_buf.is_empty() {
            // Starter with no combining marks -- just emit it.
            out.push(starter);
            return;
        }

        // Sort combining marks by CCC in place.
        self.ccc_buf.sort_in_place();

        if composes {
            compose::compose_combining_sequence_into(starter, self.ccc_buf.as_slice(), out);
        } else {
            // Decomposition only: emit starter + sorted marks.
            out.push(starter);
            for entry in self.ccc_buf.as_slice() {
                out.push(entry.ch);
            }
        }
        self.ccc_buf.clear();
    }

    /// Process a single character (after decomposition) into the accumulation state.
    ///
    /// Characters with CCC == 0 are starters. When a new starter arrives, the
    /// previous accumulation is flushed. In composition mode, starter-to-starter
    /// composition is attempted first (required for Hangul jamo L+V, LV+T).
    #[inline]
    fn feed_entry(&mut self, ch: char, ccc: u8, out: &mut String, composes: bool) {
        if ccc == 0 {
            // New starter.
            if composes && self.ccc_buf.is_empty() {
                // No intervening combining marks -- try starter-to-starter composition.
                if let Some(prev) = self.current_starter
                    && let Some(composed) = compose::compose(prev, ch)
                {
                    self.current_starter = Some(composed);
                    return;
                }
            }
            // Either not composing, has intervening marks, or composition failed.
            self.flush(out, composes);
            self.current_starter = Some(ch);
        } else {
            // Combining mark: add to buffer.
            self.ccc_buf.push(ch, ccc);
        }
    }

    /// NFD-specialized flush: no composition logic.
    #[inline]
    fn flush_nfd(&mut self, out: &mut String) {
        let starter = match self.current_starter.take() {
            Some(s) => s,
            None => {
                if !self.ccc_buf.is_empty() {
                    self.ccc_buf.sort_in_place();
                    for entry in self.ccc_buf.as_slice() {
                        out.push(entry.ch);
                    }
                    self.ccc_buf.clear();
                }
                return;
            },
        };

        // Fast path: single combining mark (most common for precomposed Latin).
        // Skip sort (unnecessary for 1 element) and avoid as_slice/clear overhead.
        if let Some(entry) = self.ccc_buf.take_single_inline() {
            out.push(starter);
            out.push(entry.ch);
            return;
        }

        if self.ccc_buf.is_empty() {
            out.push(starter);
            return;
        }

        // Multiple marks: sort and emit.
        self.ccc_buf.sort_in_place();
        out.push(starter);
        for entry in self.ccc_buf.as_slice() {
            out.push(entry.ch);
        }
        self.ccc_buf.clear();
    }

    /// NFD-specialized feed_entry: no composition checks.
    #[inline]
    fn feed_entry_nfd(&mut self, ch: char, ccc: u8, out: &mut String) {
        if ccc == 0 {
            self.flush_nfd(out);
            self.current_starter = Some(ch);
        } else {
            self.ccc_buf.push(ch, ccc);
        }
    }
}

// ---------------------------------------------------------------------------
// process_char -- decompose a char and feed entries to NormState
// ---------------------------------------------------------------------------

/// Check if a code point is a CJK Unified Ideograph (CCC=0, no decomposition,
/// no composition). These can bypass the entire decompose pipeline.
#[inline(always)]
fn is_cjk_unified(cp: u32) -> bool {
    (0x4E00..=0x9FFF).contains(&cp) || (0x3400..=0x4DBF).contains(&cp)
}

// ---------------------------------------------------------------------------
// Latin-1 supplement (U+00C0..=U+00FF) decomposition fast path
// ---------------------------------------------------------------------------
//
// NFD/NFKD on the precomposed Latin-1 supplement is by far the most common
// "needs work" payload in real-world text (`é`, `ö`, `à`, …). Each codepoint
// otherwise pays for: trie BMP lookup + has_decomposition mask + expansion
// table indexing + length read + slice. We can short-circuit the entire
// pipeline because the canonical *and* compatibility decompositions in this
// 64-codepoint range are always either (a) self-mapping (e.g. `Æ`, `Ð`,
// `×`) or (b) `(ASCII letter, single combining mark)` — never longer, never
// case-dependent. NFD == NFKD here, so a single table covers both forms.
//
// The table is indexed by `cp - 0xC0`, where `cp` is the codepoint extracted
// from the 2-byte UTF-8 sequence with `b0 == 0xC3`. Entry encoding:
//
//   * Self-mapping: `(0, 0, 0)`  (the caller falls through to the general
//     "non-decomposing" path which just bulk-passthrough's the bytes)
//   * Decomposing:  `(starter_ascii_byte, mark_cp_u16, mark_ccc_u8)`
//
// Storing only the ASCII byte for the starter (instead of a `u32` codepoint)
// keeps the table at 8 bytes/entry (with packing), which fits 64 entries in
// 8 cache lines.

/// Sentinel meaning "U+00xx maps to itself (no decomposition); use the
/// general path to bulk-passthrough this codepoint as-is".
const LATIN1_SELF_MAPPING: (u8, u16, u8) = (0, 0, 0);

/// NFD == NFKD decomposition table for U+00C0..=U+00FF.
///
/// Indexed by `cp - 0xC0`. Each `(starter_ascii, mark_cp, mark_ccc)` tuple
/// describes the canonical decomposition. `starter_ascii == 0` means the
/// codepoint does not decompose (use the general path).
///
/// Hand-derived from `UnicodeData.txt` and verified at module-init time by
/// the `latin1_table_matches_runtime_lookup` test.
#[rustfmt::skip]
static LATIN1_NFD_TABLE: [(u8, u16, u8); 0x40] = [
    // U+00C0..=U+00CF
    (b'A', 0x0300, 230), (b'A', 0x0301, 230), (b'A', 0x0302, 230), (b'A', 0x0303, 230),
    (b'A', 0x0308, 230), (b'A', 0x030A, 230), LATIN1_SELF_MAPPING,  (b'C', 0x0327, 202),
    (b'E', 0x0300, 230), (b'E', 0x0301, 230), (b'E', 0x0302, 230), (b'E', 0x0308, 230),
    (b'I', 0x0300, 230), (b'I', 0x0301, 230), (b'I', 0x0302, 230), (b'I', 0x0308, 230),
    // U+00D0..=U+00DF
    LATIN1_SELF_MAPPING,  (b'N', 0x0303, 230), (b'O', 0x0300, 230), (b'O', 0x0301, 230),
    (b'O', 0x0302, 230), (b'O', 0x0303, 230), (b'O', 0x0308, 230), LATIN1_SELF_MAPPING,
    LATIN1_SELF_MAPPING,  (b'U', 0x0300, 230), (b'U', 0x0301, 230), (b'U', 0x0302, 230),
    (b'U', 0x0308, 230), (b'Y', 0x0301, 230), LATIN1_SELF_MAPPING,  LATIN1_SELF_MAPPING,
    // U+00E0..=U+00EF
    (b'a', 0x0300, 230), (b'a', 0x0301, 230), (b'a', 0x0302, 230), (b'a', 0x0303, 230),
    (b'a', 0x0308, 230), (b'a', 0x030A, 230), LATIN1_SELF_MAPPING,  (b'c', 0x0327, 202),
    (b'e', 0x0300, 230), (b'e', 0x0301, 230), (b'e', 0x0302, 230), (b'e', 0x0308, 230),
    (b'i', 0x0300, 230), (b'i', 0x0301, 230), (b'i', 0x0302, 230), (b'i', 0x0308, 230),
    // U+00F0..=U+00FF
    LATIN1_SELF_MAPPING,  (b'n', 0x0303, 230), (b'o', 0x0300, 230), (b'o', 0x0301, 230),
    (b'o', 0x0302, 230), (b'o', 0x0303, 230), (b'o', 0x0308, 230), LATIN1_SELF_MAPPING,
    LATIN1_SELF_MAPPING,  (b'u', 0x0300, 230), (b'u', 0x0301, 230), (b'u', 0x0302, 230),
    (b'u', 0x0308, 230), (b'y', 0x0301, 230), LATIN1_SELF_MAPPING,  (b'y', 0x0308, 230),
];

/// NFD/NFKD fast path for the Latin-1 supplement (U+00C0..=U+00FF).
///
/// Only valid when the byte at `byte_pos` is `0xC3` (the only UTF-8 leading
/// byte that emits codepoints in this range). Reads `b1`, indexes the table,
/// and either short-circuits the decode-or-passthrough decision or returns
/// `None` for self-mapping codepoints (which the caller falls through to
/// the general non-decomposing path for).
///
/// Returns `Some((starter, mark_cp, mark_ccc))` when the codepoint decomposes
/// to a single ASCII starter + single combining mark.
///
/// # Safety
///
/// `byte_pos + 1 < len` and the input must be valid UTF-8 starting with
/// `0xC3` at `byte_pos` — both guaranteed by the SIMD bit-walk's leading-byte
/// filter and the original `&str` input invariant.
#[inline(always)]
unsafe fn latin1_supplement_nfd(bytes: *const u8, byte_pos: usize) -> Option<(u8, char, u8)> {
    // SAFETY: caller guarantees `byte_pos + 1 < len`.
    let b1 = unsafe { *bytes.add(byte_pos + 1) };
    let idx = (b1 & 0x3F) as usize; // b1 = 0x80 | (cp & 0x3F); cp = 0xC0 | (b1 & 0x3F).
    let entry = LATIN1_NFD_TABLE[idx];
    if entry.0 == 0 {
        return None;
    }
    // SAFETY: every non-self-mapping entry stores a valid combining-mark
    // codepoint (all in the U+0300..=U+0327 range, well-formed scalars).
    let mark = unsafe { char::from_u32_unchecked(entry.1 as u32) };
    Some((entry.0, mark, entry.2))
}

/// Decompose a character and feed each resulting entry into the accumulation state.
///
/// Uses a single trie lookup with passthrough fast-paths for non-decomposing
/// characters, avoiding the full decomposition pipeline for the common case.
#[inline]
fn process_char(
    ch: char,
    state: &mut NormState,
    out: &mut String,
    form: Form,
    decomp_buf: &mut CccBuffer,
) {
    let cp = ch as u32;

    // Fast path: CJK ideographs never decompose, have CCC=0, and never
    // participate in canonical composition. No trie lookup needed.
    if cp >= 0x3400 && is_cjk_unified(cp) {
        state.flush(out, form.composes());
        state.current_starter = Some(ch);
        return;
    }

    // Hangul syllables: algorithmic decomposition, no trie lookup needed.
    if hangul::is_hangul_syllable(ch) {
        let (l, v, t) = hangul::decompose_hangul(ch);
        state.feed_entry(l, 0, out, form.composes());
        state.feed_entry(v, 0, out, form.composes());
        if let Some(t_char) = t {
            state.feed_entry(t_char, 0, out, form.composes());
        }
        return;
    }

    // Single trie lookup for both passthrough check and decomposition.
    let trie_value = tables::raw_decomp_trie_value(ch, form.decomp_form());

    // Non-decomposing character: extract CCC and feed directly.
    // This covers both starters (CCC=0) and combining marks (CCC>0)
    // that map to themselves, skipping the full decompose pipeline.
    if !tables::has_decomposition(trie_value) {
        let ccc = tables::ccc_from_trie_value(trie_value);
        state.feed_entry(ch, ccc, out, form.composes());
        return;
    }

    // Character has a decomposition: decode from the pre-looked-up trie value.
    decomp_buf.clear();
    decompose::decompose_from_trie_value(ch, trie_value, decomp_buf, form.decomp_form());
    for entry in decomp_buf.as_slice() {
        state.feed_entry(entry.ch, entry.ccc, out, form.composes());
    }
}

// ---------------------------------------------------------------------------
// Unified per-codepoint decode pipeline (Component D)
// ---------------------------------------------------------------------------
//
// On dense scripts (CJK, Hangul, emoji, Arabic) every byte in a 64-byte SIMD
// chunk is a "hit", and the per-codepoint decode/CCC/decompose calls dominate
// runtime. The unified `decode_at` performs UTF-8 decode, CCC extraction, and
// decomposition lookup in one pass, sharing the bounds check and table-pointer
// derivation. `process_codepoint` then dispatches form-specifically on the
// resulting `DecodedCodepoint` without re-doing any of that work.

/// Discriminator describing what kind of decomposition (if any) was looked up
/// for a codepoint. The variant determines which decomposition table the
/// `decomp` slice came from (or whether it is empty / singleton-encoded).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DecompKind {
    /// Codepoint has no decomposition mapping in the relevant form. The
    /// codepoint passes through with its own CCC.
    None,
    /// Decomposition was looked up via the canonical (NFC/NFD) trie.
    Canonical,
    /// Decomposition was looked up via the compatibility (NFKC/NFKD) trie.
    Compat,
}

/// Result of a unified UTF-8 + CCC + decomposition lookup at a byte position.
///
/// `decomp` is the expansion slice from the canonical/compat table when the
/// codepoint has an expansion-form decomposition; for singleton decompositions
/// (which target a single BMP codepoint encoded inside the trie value), the
/// slice is empty and `tv` carries the trie value so the singleton can be
/// resolved without a second lookup.
struct DecodedCodepoint {
    /// The decoded codepoint.
    cp: u32,
    /// UTF-8 byte length, in `1..=4`.
    cp_len: u8,
    /// Canonical Combining Class for this codepoint (0 for starters).
    ccc: u8,
    /// Discriminator for the `decomp` field interpretation.
    decomp_kind: DecompKind,
    /// Expansion data slice (empty if `decomp_kind == None` or if the
    /// decomposition is a singleton — see `tv` for the singleton case).
    decomp: &'static [u32],
    /// Raw trie value from the relevant decomposition trie. Used to fall back
    /// to the existing singleton path without a second lookup.
    tv: u32,
}

/// Single-pass UTF-8 decode + CCC + decomposition lookup at byte index `idx`.
///
/// Performs the bounds check and table-pointer derivation once for all four
/// pieces of information, replacing four separate sub-calls each with their
/// own checks.
///
/// # Safety
///
/// - `bytes` must point to at least `len` valid bytes.
/// - `idx` must be a valid index `< len` and must point to a UTF-8 leading
///   byte (not a continuation byte). Both invariants are guaranteed by the
///   SIMD scanner contract on the bit-walk: bits in the chunk mask only
///   correspond to bytes `>= bound`, and our bound rules out the
///   `0x80..=0xBF` range from being a leading byte.
/// - The bytes at `idx..idx + cp_len` must form a valid UTF-8 sequence (true
///   because `bytes` originates from a valid `&str`).
#[inline(always)]
unsafe fn decode_at(bytes: *const u8, idx: usize, len: usize, form: Form) -> DecodedCodepoint {
    debug_assert!(idx < len);
    // SAFETY: `idx < len` and `bytes` is valid for `len` bytes.
    let b0 = unsafe { *bytes.add(idx) };
    let cp_len = utf8::utf8_char_width(b0);
    debug_assert!(cp_len > 0, "decode_at called on continuation/invalid byte");
    debug_assert!(idx + cp_len <= len, "UTF-8 sequence runs past end of input");

    // Decode the codepoint. The branch arms correspond to the four possible
    // UTF-8 widths; `cp_len == 0` is impossible because the SIMD scanner only
    // surfaces leading bytes (any continuation byte is filtered out by the
    // caller before invoking `decode_at`).
    let cp = match cp_len {
        1 => b0 as u32,
        2 => {
            // SAFETY: `idx + 1 < len` because `cp_len == 2` and the input is
            // valid UTF-8 with at least `cp_len` bytes available.
            let b1 = unsafe { *bytes.add(idx + 1) } as u32;
            ((b0 as u32 & 0x1F) << 6) | (b1 & 0x3F)
        },
        3 => {
            // SAFETY: as above with `cp_len == 3`.
            let b1 = unsafe { *bytes.add(idx + 1) } as u32;
            let b2 = unsafe { *bytes.add(idx + 2) } as u32;
            ((b0 as u32 & 0x0F) << 12) | ((b1 & 0x3F) << 6) | (b2 & 0x3F)
        },
        4 => {
            // SAFETY: as above with `cp_len == 4`.
            let b1 = unsafe { *bytes.add(idx + 1) } as u32;
            let b2 = unsafe { *bytes.add(idx + 2) } as u32;
            let b3 = unsafe { *bytes.add(idx + 3) } as u32;
            ((b0 as u32 & 0x07) << 18) | ((b1 & 0x3F) << 12) | ((b2 & 0x3F) << 6) | (b3 & 0x3F)
        },
        // SAFETY: `cp_len` is one of {1,2,3,4} for a valid UTF-8 leading byte;
        // the SIMD bit-walk only surfaces leading bytes by contract.
        _ => unsafe { core::hint::unreachable_unchecked() },
    };

    // Single trie lookup for both CCC and decomposition. Use the unchecked
    // supplementary path for cp >= 0x10000 (emoji etc.) to skip the BMP
    // branch in `CodePointTrie::get`.
    let decomp_form = form.decomp_form();
    let tv = if cp >= 0x10000 {
        // SAFETY: `cp` is a valid supplementary code point from a valid char.
        unsafe { tables::raw_decomp_trie_value_supplementary(cp, decomp_form) }
    } else {
        // SAFETY: BMP path — `cp` is a valid Unicode scalar value.
        let ch = unsafe { char::from_u32_unchecked(cp) };
        tables::raw_decomp_trie_value(ch, decomp_form)
    };
    let ccc = tables::ccc_from_trie_value(tv);

    let (decomp_kind, decomp) = if !tables::has_decomposition(tv) {
        (DecompKind::None, &[][..])
    } else {
        let kind = match decomp_form {
            DecompForm::Canonical => DecompKind::Canonical,
            DecompForm::Compatible => DecompKind::Compat,
        };
        // Expansion if present; empty slice marks a singleton (handled via tv).
        let slice = tables::expansion_data_from_trie_value(tv, decomp_form).unwrap_or(&[]);
        (kind, slice)
    };

    DecodedCodepoint {
        cp,
        cp_len: cp_len as u8,
        ccc,
        decomp_kind,
        decomp,
        tv,
    }
}

/// Feed the entries of an expansion-form decomposition into `state`. Each
/// entry packs `(ccc, codepoint)` per `EXPANSION_CCC_SHIFT` / `EXPANSION_CP_MASK`.
///
/// In NFD/NFKD mode (`!composes`), specializes the 2-entry "starter +
/// combining mark" expansion (the common precomposed-Latin / Greek / Cyrillic
/// case) to bypass per-entry `feed_entry_nfd` overhead.
#[inline(always)]
fn feed_expansion(decomp: &'static [u32], state: &mut NormState, out: &mut String, composes: bool) {
    if !composes && decomp.len() == 2 {
        let e0 = decomp[0];
        let ccc0 = (e0 >> tables::EXPANSION_CCC_SHIFT) as u8;
        if ccc0 == 0 {
            // Starter + (any second entry). Hot path on precomposed Latin.
            state.flush_nfd(out);
            let cp0 = e0 & tables::EXPANSION_CP_MASK;
            debug_assert!(cp0 <= 0x10FFFF && !(0xD800..=0xDFFF).contains(&cp0));
            // SAFETY: expansion data targets are valid Unicode scalar values.
            state.current_starter = Some(unsafe { char::from_u32_unchecked(cp0) });
            let e1 = decomp[1];
            let cp1 = e1 & tables::EXPANSION_CP_MASK;
            let ccc1 = (e1 >> tables::EXPANSION_CCC_SHIFT) as u8;
            debug_assert!(cp1 <= 0x10FFFF && !(0xD800..=0xDFFF).contains(&cp1));
            // SAFETY: expansion data targets are valid Unicode scalar values.
            let ch1 = unsafe { char::from_u32_unchecked(cp1) };
            if ccc1 != 0 {
                state.ccc_buf.push(ch1, ccc1);
            } else {
                state.feed_entry_nfd(ch1, 0, out);
            }
            return;
        }
    }
    for &entry in decomp {
        let cp = entry & tables::EXPANSION_CP_MASK;
        let ccc = (entry >> tables::EXPANSION_CCC_SHIFT) as u8;
        debug_assert!(cp <= 0x10FFFF && !(0xD800..=0xDFFF).contains(&cp));
        // SAFETY: expansion data is generated from valid Unicode scalar values.
        let exp_ch = unsafe { char::from_u32_unchecked(cp) };
        if composes {
            state.feed_entry(exp_ch, ccc, out, true);
        } else {
            state.feed_entry_nfd(exp_ch, ccc, out);
        }
    }
}

/// Cold path: feed a singleton decomposition (one BMP codepoint embedded in
/// the trie value) into `state`. Singletons are rare on dense scripts where
/// the SIMD-hit branch dominates, so we mark this branch `#[cold]` to keep
/// the hot path (passthrough + expansion) tighter.
#[cold]
#[inline(never)]
fn feed_singleton(tv: u32, state: &mut NormState, out: &mut String, composes: bool) {
    let info = tv & 0xFFFF;
    debug_assert!(info <= 0xD7FF || (0xE000..=0xFFFF).contains(&info));
    // SAFETY: singleton target is a valid BMP codepoint by table construction.
    let decomposed = unsafe { char::from_u32_unchecked(info) };
    let ccc = if info <= 0x7F {
        0
    } else {
        tables::lookup_ccc(decomposed)
    };
    if composes {
        state.feed_entry(decomposed, ccc, out, true);
    } else {
        state.feed_entry_nfd(decomposed, ccc, out);
    }
}

/// Helper: feed a combining mark (CCC > 0) into `state`. Combining marks
/// drive the CCC reorder chain inside `NormState`. On dense scripts (CJK /
/// Hangul / emoji) where the SIMD-hit branch dominates this is rare, but on
/// scripts dominated by combining marks (Arabic, the `worst_case` base+marks
/// stream) it is hit on every codepoint, so we keep it `#[inline]` so it
/// folds into the bit-walk and benefits from `feed_entry_nfd`'s own inlining.
#[inline]
fn feed_combining_mark(ch: char, ccc: u8, state: &mut NormState, out: &mut String, composes: bool) {
    if composes {
        state.feed_entry(ch, ccc, out, true);
    } else {
        state.feed_entry_nfd(ch, ccc, out);
    }
}

/// Drive a decoded codepoint through the form-specific accumulation state.
///
/// Replaces the four-call sub-pipeline (decode → ccc → decompose → process)
/// at each bit-walk position with a single dispatch on `DecodedCodepoint`.
#[inline(always)]
fn process_codepoint(dc: &DecodedCodepoint, state: &mut NormState, out: &mut String, form: Form) {
    let composes = form.composes();
    match dc.decomp_kind {
        DecompKind::None => {
            // No decomposition: feed the codepoint with its CCC.
            // SAFETY: `cp` came from a valid `&str`, so it's a valid scalar.
            let ch = unsafe { char::from_u32_unchecked(dc.cp) };
            if dc.ccc == 0 {
                if composes {
                    state.feed_entry(ch, 0, out, true);
                } else {
                    state.feed_entry_nfd(ch, 0, out);
                }
            } else {
                feed_combining_mark(ch, dc.ccc, state, out, composes);
            }
        },
        DecompKind::Canonical | DecompKind::Compat => {
            if !dc.decomp.is_empty() {
                feed_expansion(dc.decomp, state, out, composes);
            } else {
                feed_singleton(dc.tv, state, out, composes);
            }
        },
    }
}

/// Compose-mode passthrough flush. Called from both the chunk loop and scalar
/// tail after `state.flush(out, true)` when `composes == true`. Peeks at the
/// upcoming codepoint via its already-fetched trie value (`next_tv`) and
/// decides whether to copy the whole `pass` run verbatim or feed the final
/// ASCII starter through `NormState` so subsequent combining marks can still
/// see it. The caller passes `next_tv` from the unified `decode_at` so we
/// avoid a redundant trie lookup on every SIMD-hit codepoint (notably the
/// dense emoji / Arabic / Hangul streams where this is hit per codepoint).
#[inline(always)]
fn flush_compose_passthrough(pass: &str, next_tv: u32, state: &mut NormState, out: &mut String) {
    if tables::needs_starter_shadow(next_tv) {
        let n = pass.len();
        if n > 1 {
            out.push_str(&pass[..n - 1]);
        }
        let last_ch = pass.as_bytes()[n - 1] as char;
        state.feed_entry(last_ch, 0, out, true);
    } else {
        out.push_str(pass);
    }
}

// ---------------------------------------------------------------------------
// normalize_scalar -- fallback for short inputs
// ---------------------------------------------------------------------------

/// Normalize a string using pure scalar processing (no SIMD).
fn normalize_scalar<'a>(input: &'a str, form: Form) -> Cow<'a, str> {
    if input.is_empty() {
        return Cow::Borrowed(input);
    }

    // Quick-check: if the string is definitely already normalized, return early.
    if form.quick_check(input) == quick_check::IsNormalized::Yes {
        return Cow::Borrowed(input);
    }

    let mut out = String::with_capacity(input.len());
    let mut state = NormState::new();
    let mut decomp_buf = CccBuffer::new();

    for ch in input.chars() {
        process_char(ch, &mut state, &mut out, form, &mut decomp_buf);
    }

    // Flush any remaining state.
    state.flush(&mut out, form.composes());

    if out == input {
        Cow::Borrowed(input)
    } else {
        Cow::Owned(out)
    }
}

// ---------------------------------------------------------------------------
// normalize_impl -- main SIMD-accelerated loop
// ---------------------------------------------------------------------------

/// Core normalization function.
///
/// Uses SIMD scanning for inputs >= 64 bytes, with scalar fallback for shorter
/// inputs and tails. Returns `Cow::Borrowed` if the input was already normalized.
///
/// `#[inline]` is intentional: `Form` is dispatched as a runtime parameter
/// from four single-form public entry points (`Nfc`/`Nfd`/`Nfkc`/`Nfkd`), and
/// inlining lets the compiler fold all `match form { ... }` branches inside
/// `process_codepoint`, `feed_expansion`, `decode_at`, etc. into the constant
/// chosen at the call site. This collapses the inner loop's per-codepoint
/// `composes` / `decomp_form()` checks to a no-op. The compile-time cost is
/// four monomorphic copies of the function (~2× current `.text`), which we
/// accept for the per-codepoint speedup on combining-mark-dense fixtures
/// (`worst_case`, `already_nfc`).
#[inline]
fn normalize_impl<'a>(input: &'a str, form: Form) -> Cow<'a, str> {
    let bytes = input.as_bytes();
    let len = bytes.len();

    // Short inputs: use scalar path directly (includes quick_check).
    if len < 64 {
        return normalize_scalar(input, form);
    }

    // Single upfront quick-check. If definitely normalized, return early.
    let qc = form.quick_check(input);
    if qc == quick_check::IsNormalized::Yes {
        return Cow::Borrowed(input);
    }

    // QC = No or Maybe: allocate and normalize.
    let bound = form.passthrough_bound();
    let composes = form.composes();
    let mut out = String::with_capacity(form.estimated_capacity(len));
    let mut last_written: usize = 0;
    let mut state = NormState::new();

    let mut pos: usize = 0;
    let ptr = bytes.as_ptr();

    // SIMD chunk loop.
    while pos + 64 <= len {
        let chunk_start = pos;

        // SAFETY: pos + 64 <= len, so ptr.add(pos) is valid for 64 bytes.
        // Prefetch pointers use wrapping_add because they may exceed the
        // allocation; prefetch is a non-faulting hint on all architectures.
        let mask = unsafe {
            let prefetch_l1 =
                ptr.wrapping_add(pos + prefetch::PREFETCH_L1_DISTANCE * prefetch::CHUNK_SIZE);
            let prefetch_l2 =
                ptr.wrapping_add(pos + prefetch::PREFETCH_L2_DISTANCE * prefetch::CHUNK_SIZE);
            simd::scan_and_prefetch(ptr.add(pos), prefetch_l1, prefetch_l2, bound)
        };

        // Prefetch the output buffer write-head to overlap write-allocate
        // fills with the SIMD scanner read on the source. Guarded against the
        // reallocation boundary: if the prefetched line would land past the
        // current capacity, skip it (the next push_str will realloc anyway).
        unsafe {
            let write_head = out.len();
            let distance = prefetch::PREFETCH_L1_DISTANCE * prefetch::CHUNK_SIZE;
            if write_head + distance <= out.capacity() {
                prefetch::prefetch_write(out.as_ptr().wrapping_add(write_head + distance));
            }
        }

        if mask == 0 {
            // All passthrough: no bytes >= bound in this chunk.
            pos += 64;
            continue;
        }

        // Walk set bits in the mask. Each set bit marks a byte >= bound; the
        // unified `decode_at` derives UTF-8 width, codepoint, CCC, and decomp
        // in one pass, then `process_codepoint` dispatches form-specifically.
        let mut chunk_mask = mask;
        while chunk_mask != 0 {
            let bit_pos = chunk_mask.trailing_zeros() as usize;
            chunk_mask &= chunk_mask.wrapping_sub(1); // clear lowest set bit

            let byte_pos = chunk_start + bit_pos;

            // Skip if we already processed past this position (multi-byte char from previous bit).
            if byte_pos < last_written {
                continue;
            }

            // Skip continuation bytes -- they belong to a char whose leading byte
            // was already processed.
            if utf8::is_continuation_byte(bytes[byte_pos]) {
                continue;
            }

            // Latin-1 supplement NFD/NFKD fast path: when the leading byte is
            // 0xC3 (only emits U+00C0..=U+00FF), short-circuit decode + trie
            // lookup with a 64-entry table. Most precomposed Latin-1 letters
            // decompose to a single ASCII starter + single combining mark,
            // which we can emit directly without going through process_codepoint.
            if !composes && bytes[byte_pos] == 0xC3 {
                // SAFETY: byte_pos + 1 < len because UTF-8 validity guarantees
                // the 0xC3 leading byte is followed by exactly one continuation.
                if let Some((starter, mark, mark_ccc)) =
                    unsafe { latin1_supplement_nfd(ptr, byte_pos) }
                {
                    if byte_pos > last_written {
                        state.flush_nfd(&mut out);
                        out.push_str(&input[last_written..byte_pos]);
                    }
                    last_written = byte_pos + 2;
                    state.flush_nfd(&mut out);
                    // SAFETY: starter is an ASCII byte (< 0x80) by table construction.
                    out.push(starter as char);
                    state.ccc_buf.push(mark, mark_ccc);
                    continue;
                }
                // Self-mapping: fall through to the general non-decomposing
                // bulk-passthrough path below.
            }

            // SAFETY: `byte_pos < len` (bit_pos < 64, chunk_start + 64 <= len)
            // and the byte at `byte_pos` is a UTF-8 leading byte (continuation
            // filter above). The input came from a valid `&str`, so the full
            // sequence of `cp_len` bytes is guaranteed to be valid UTF-8.
            let dc = unsafe { decode_at(ptr, byte_pos, len, form) };
            let width = dc.cp_len as usize;

            // Extended passthrough for decomposition-only forms (NFD/NFKD):
            // Non-decomposing starters (CCC=0) produce identical output, so
            // they can be bulk-copied with surrounding passthrough bytes,
            // avoiding per-character NormState flush + push overhead.
            if !composes {
                // Hangul syllables: algorithmic decomposition (the trie reports
                // them as no-decomp), write jamo directly to output. Must
                // precede the no-decomp passthrough check below.
                if (hangul::S_BASE..hangul::S_BASE + hangul::S_COUNT).contains(&dc.cp) {
                    if byte_pos > last_written {
                        state.flush_nfd(&mut out);
                        out.push_str(&input[last_written..byte_pos]);
                    }
                    last_written = byte_pos + width;
                    state.flush_nfd(&mut out);
                    // SAFETY: dc.cp is a valid Hangul syllable codepoint.
                    let ch = unsafe { char::from_u32_unchecked(dc.cp) };
                    let (l, v, t) = hangul::decompose_hangul(ch);
                    out.push(l);
                    out.push(v);
                    if let Some(t_char) = t {
                        out.push(t_char);
                    }
                    continue;
                }
                // Non-decomposing starter: bulk-passthrough.
                if dc.decomp_kind == DecompKind::None && dc.ccc == 0 {
                    continue;
                }
                // Needs work: copy passthrough, then process via unified pipeline.
                if byte_pos > last_written {
                    state.flush_nfd(&mut out);
                    out.push_str(&input[last_written..byte_pos]);
                }
                last_written = byte_pos + width;
                process_codepoint(&dc, &mut state, &mut out, form);
                continue;
            }

            // Compose mode: copy any passthrough bytes between last_written and
            // this position. Flush NormState first: it may hold a buffered
            // starter that must appear *before* the passthrough run.
            if byte_pos > last_written {
                state.flush(&mut out, composes);
                let pass = &input[last_written..byte_pos];
                // Reuse the trie value already fetched by `decode_at` instead
                // of re-running `raw_decomp_trie_value` on `dc.cp`.
                flush_compose_passthrough(pass, dc.tv, &mut state, &mut out);
            }

            last_written = byte_pos + width;
            process_codepoint(&dc, &mut state, &mut out, form);
        }

        pos += 64;
    }

    // Scalar tail: remaining bytes after the last full chunk.
    if pos < len {
        // Check if the tail has any non-passthrough bytes.
        let tail_has_work = bytes[pos..].iter().any(|&b| b >= bound);

        if tail_has_work {
            // Process remaining bytes character-by-character via the unified
            // `decode_at` + `process_codepoint` pipeline (same shape as the
            // SIMD bit-walk above).
            let mut tail_pos = pos;
            while tail_pos < len {
                if tail_pos < last_written {
                    tail_pos += 1;
                    continue;
                }

                if utf8::is_continuation_byte(bytes[tail_pos]) {
                    tail_pos += 1;
                    continue;
                }

                // Latin-1 supplement NFD/NFKD fast path (mirrors bit-walk).
                if !composes && bytes[tail_pos] == 0xC3 {
                    // SAFETY: tail_pos + 1 < len by UTF-8 validity.
                    if let Some((starter, mark, mark_ccc)) =
                        unsafe { latin1_supplement_nfd(ptr, tail_pos) }
                    {
                        if tail_pos > last_written {
                            state.flush_nfd(&mut out);
                            out.push_str(&input[last_written..tail_pos]);
                        }
                        last_written = tail_pos + 2;
                        state.flush_nfd(&mut out);
                        out.push(starter as char);
                        state.ccc_buf.push(mark, mark_ccc);
                        tail_pos += 2;
                        continue;
                    }
                }

                // SAFETY: `tail_pos < len` and the byte is a UTF-8 leading
                // byte (continuation filter above). Input is a valid `&str`.
                let dc = unsafe { decode_at(ptr, tail_pos, len, form) };
                let width = dc.cp_len as usize;

                // Extended passthrough (NFD/NFKD): bulk-copy non-decomposing
                // starters; algorithmic Hangul written directly to output.
                if !composes {
                    if (hangul::S_BASE..hangul::S_BASE + hangul::S_COUNT).contains(&dc.cp) {
                        if tail_pos > last_written {
                            state.flush_nfd(&mut out);
                            out.push_str(&input[last_written..tail_pos]);
                        }
                        last_written = tail_pos + width;
                        state.flush_nfd(&mut out);
                        // SAFETY: dc.cp is a valid Hangul syllable codepoint.
                        let ch = unsafe { char::from_u32_unchecked(dc.cp) };
                        let (l, v, t) = hangul::decompose_hangul(ch);
                        out.push(l);
                        out.push(v);
                        if let Some(t_char) = t {
                            out.push(t_char);
                        }
                        tail_pos += width;
                        continue;
                    }
                    if dc.decomp_kind == DecompKind::None && dc.ccc == 0 {
                        tail_pos += width;
                        continue;
                    }
                    if tail_pos > last_written {
                        state.flush_nfd(&mut out);
                        out.push_str(&input[last_written..tail_pos]);
                    }
                    last_written = tail_pos + width;
                    process_codepoint(&dc, &mut state, &mut out, form);
                    tail_pos += width;
                    continue;
                }

                // Compose mode: copy passthrough bytes before this char.
                if tail_pos > last_written {
                    state.flush(&mut out, composes);
                    let pass = &input[last_written..tail_pos];
                    // Reuse the trie value already fetched by `decode_at`.
                    flush_compose_passthrough(pass, dc.tv, &mut state, &mut out);
                }

                last_written = tail_pos + width;
                process_codepoint(&dc, &mut state, &mut out, form);

                tail_pos += width;
            }
        }
    }

    // Flush any remaining state.
    if composes {
        state.flush(&mut out, true);
    } else {
        state.flush_nfd(&mut out);
    }

    // Copy any trailing passthrough bytes.
    if last_written < len {
        out.push_str(&input[last_written..len]);
    }

    // For the Maybe case (NFC/NFKC only), normalization might not have changed
    // anything. Check and return Borrowed if so.
    if qc == quick_check::IsNormalized::Maybe && out == input {
        Cow::Borrowed(input)
    } else {
        Cow::Owned(out)
    }
}

// ---------------------------------------------------------------------------
// Public normalizer types
// ---------------------------------------------------------------------------

/// NFC normalizer: Canonical Decomposition, followed by Canonical Composition.
pub struct NfcNormalizer;

/// NFD normalizer: Canonical Decomposition.
pub struct NfdNormalizer;

/// NFKC normalizer: Compatibility Decomposition, followed by Canonical Composition.
pub struct NfkcNormalizer;

/// NFKD normalizer: Compatibility Decomposition.
pub struct NfkdNormalizer;

impl Default for NfcNormalizer {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for NfdNormalizer {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for NfkcNormalizer {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for NfkdNormalizer {
    fn default() -> Self {
        Self::new()
    }
}

impl NfcNormalizer {
    /// Create a new NFC normalizer.
    pub fn new() -> Self {
        NfcNormalizer
    }

    /// Run the NFC quick-check algorithm on `input`.
    pub fn quick_check(&self, input: &str) -> crate::quick_check::IsNormalized {
        quick_check::quick_check_nfc(input)
    }

    /// Normalize the input string to NFC form.
    ///
    /// Returns `Cow::Borrowed` if the input is already in NFC.
    pub fn normalize<'a>(&self, input: &'a str) -> Cow<'a, str> {
        normalize_impl(input, Form::Nfc)
    }

    /// Normalize the input string to NFC form, appending to `out`.
    ///
    /// Returns `true` if the input was already normalized (nothing was modified).
    pub fn normalize_to(&self, input: &str, out: &mut String) -> bool {
        let result = normalize_impl(input, Form::Nfc);
        let already_normalized = matches!(&result, Cow::Borrowed(_));
        out.push_str(&result);
        already_normalized
    }

    /// Check if the input is already in NFC form.
    pub fn is_normalized(&self, input: &str) -> bool {
        quick_check::is_normalized_nfc(input)
    }
}

impl NfdNormalizer {
    /// Create a new NFD normalizer.
    pub fn new() -> Self {
        NfdNormalizer
    }

    /// Run the NFD quick-check algorithm on `input`.
    pub fn quick_check(&self, input: &str) -> crate::quick_check::IsNormalized {
        quick_check::quick_check_nfd(input)
    }

    /// Normalize the input string to NFD form.
    ///
    /// Returns `Cow::Borrowed` if the input is already in NFD.
    pub fn normalize<'a>(&self, input: &'a str) -> Cow<'a, str> {
        normalize_impl(input, Form::Nfd)
    }

    /// Normalize the input string to NFD form, appending to `out`.
    ///
    /// Returns `true` if the input was already normalized (nothing was modified).
    pub fn normalize_to(&self, input: &str, out: &mut String) -> bool {
        let result = normalize_impl(input, Form::Nfd);
        let already_normalized = matches!(&result, Cow::Borrowed(_));
        out.push_str(&result);
        already_normalized
    }

    /// Check if the input is already in NFD form.
    pub fn is_normalized(&self, input: &str) -> bool {
        quick_check::is_normalized_nfd(input)
    }
}

impl NfkcNormalizer {
    /// Create a new NFKC normalizer.
    pub fn new() -> Self {
        NfkcNormalizer
    }

    /// Run the NFKC quick-check algorithm on `input`.
    pub fn quick_check(&self, input: &str) -> crate::quick_check::IsNormalized {
        quick_check::quick_check_nfkc(input)
    }

    /// Normalize the input string to NFKC form.
    ///
    /// Returns `Cow::Borrowed` if the input is already in NFKC.
    pub fn normalize<'a>(&self, input: &'a str) -> Cow<'a, str> {
        normalize_impl(input, Form::Nfkc)
    }

    /// Normalize the input string to NFKC form, appending to `out`.
    ///
    /// Returns `true` if the input was already normalized (nothing was modified).
    pub fn normalize_to(&self, input: &str, out: &mut String) -> bool {
        let result = normalize_impl(input, Form::Nfkc);
        let already_normalized = matches!(&result, Cow::Borrowed(_));
        out.push_str(&result);
        already_normalized
    }

    /// Check if the input is already in NFKC form.
    pub fn is_normalized(&self, input: &str) -> bool {
        quick_check::is_normalized_nfkc(input)
    }
}

impl NfkdNormalizer {
    /// Create a new NFKD normalizer.
    pub fn new() -> Self {
        NfkdNormalizer
    }

    /// Run the NFKD quick-check algorithm on `input`.
    pub fn quick_check(&self, input: &str) -> crate::quick_check::IsNormalized {
        quick_check::quick_check_nfkd(input)
    }

    /// Normalize the input string to NFKD form.
    ///
    /// Returns `Cow::Borrowed` if the input is already in NFKD.
    pub fn normalize<'a>(&self, input: &'a str) -> Cow<'a, str> {
        normalize_impl(input, Form::Nfkd)
    }

    /// Normalize the input string to NFKD form, appending to `out`.
    ///
    /// Returns `true` if the input was already normalized (nothing was modified).
    pub fn normalize_to(&self, input: &str, out: &mut String) -> bool {
        let result = normalize_impl(input, Form::Nfkd);
        let already_normalized = matches!(&result, Cow::Borrowed(_));
        out.push_str(&result);
        already_normalized
    }

    /// Check if the input is already in NFKD form.
    pub fn is_normalized(&self, input: &str) -> bool {
        quick_check::is_normalized_nfkd(input)
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::borrow::Cow;
    use alloc::string::String;
    use alloc::vec::Vec;

    // ===================================================================
    // 0. Latin-1 NFD/NFKD fast-path table verification
    // ===================================================================

    #[test]
    fn latin1_table_matches_runtime_lookup_nfd() {
        // For every codepoint in U+00C0..=U+00FF, verify that our hand-rolled
        // table entry produces the same NFD output as feeding the codepoint
        // through the general trie-driven pipeline.
        for cp in 0xC0u32..=0xFF {
            let ch = char::from_u32(cp).unwrap();
            let mut buf = String::new();
            buf.push(ch);
            let general: Cow<'_, str> = normalize_impl(&buf, Form::Nfd);

            let entry = LATIN1_NFD_TABLE[(cp - 0xC0) as usize];
            let mut fast = String::new();
            if entry.0 == 0 {
                // Self-mapping: must equal input.
                fast.push(ch);
            } else {
                fast.push(entry.0 as char);
                fast.push(char::from_u32(entry.1 as u32).unwrap());
            }
            assert_eq!(
                &*general, fast,
                "NFD mismatch for U+{:04X}: trie={:?} table={:?}",
                cp, &*general, fast
            );
        }
    }

    #[test]
    fn latin1_table_matches_runtime_lookup_nfkd() {
        // NFKD == NFD on this range; verify explicitly.
        for cp in 0xC0u32..=0xFF {
            let ch = char::from_u32(cp).unwrap();
            let mut buf = String::new();
            buf.push(ch);
            let nfd: Cow<'_, str> = normalize_impl(&buf, Form::Nfd);
            let nfkd: Cow<'_, str> = normalize_impl(&buf, Form::Nfkd);
            assert_eq!(
                &*nfd, &*nfkd,
                "NFD/NFKD diverge for U+{:04X}: nfd={:?} nfkd={:?}",
                cp, &*nfd, &*nfkd
            );
        }
    }

    // ===================================================================
    // 1. Form enum methods
    // ===================================================================

    #[test]
    fn passthrough_bound_all_forms_return_0xc0() {
        assert_eq!(Form::Nfc.passthrough_bound(), 0xC0);
        assert_eq!(Form::Nfd.passthrough_bound(), 0xC0);
        assert_eq!(Form::Nfkc.passthrough_bound(), 0xC0);
        assert_eq!(Form::Nfkd.passthrough_bound(), 0xC0);
    }

    #[test]
    fn composes_nfc_nfkc_true_nfd_nfkd_false() {
        assert!(Form::Nfc.composes());
        assert!(Form::Nfkc.composes());
        assert!(!Form::Nfd.composes());
        assert!(!Form::Nfkd.composes());
    }

    #[test]
    fn decomp_form_canonical_vs_compatible() {
        assert_eq!(Form::Nfc.decomp_form(), DecompForm::Canonical);
        assert_eq!(Form::Nfd.decomp_form(), DecompForm::Canonical);
        assert_eq!(Form::Nfkc.decomp_form(), DecompForm::Compatible);
        assert_eq!(Form::Nfkd.decomp_form(), DecompForm::Compatible);
    }

    #[test]
    fn estimated_capacity_nfc_nfkc_same_nfd_nfkd_larger() {
        let input_len = 100;
        assert_eq!(Form::Nfc.estimated_capacity(input_len), 100);
        assert_eq!(Form::Nfkc.estimated_capacity(input_len), 100);
        assert_eq!(Form::Nfd.estimated_capacity(input_len), 150);
        assert_eq!(Form::Nfkd.estimated_capacity(input_len), 150);
    }

    #[test]
    fn estimated_capacity_zero_length() {
        assert_eq!(Form::Nfc.estimated_capacity(0), 0);
        assert_eq!(Form::Nfd.estimated_capacity(0), 0);
    }

    #[test]
    fn quick_check_ascii_is_yes_for_all_forms() {
        let ascii = "Hello, World!";
        assert_eq!(Form::Nfc.quick_check(ascii), quick_check::IsNormalized::Yes);
        assert_eq!(Form::Nfd.quick_check(ascii), quick_check::IsNormalized::Yes);
        assert_eq!(
            Form::Nfkc.quick_check(ascii),
            quick_check::IsNormalized::Yes
        );
        assert_eq!(
            Form::Nfkd.quick_check(ascii),
            quick_check::IsNormalized::Yes
        );
    }

    // ===================================================================
    // 2. NormState state machine
    // ===================================================================

    #[test]
    fn normstate_new_has_no_starter_empty_ccc_buf() {
        let state = NormState::new();
        assert!(state.current_starter.is_none());
        assert!(state.ccc_buf.is_empty());
    }

    #[test]
    fn feed_entry_single_starter_sets_current_starter() {
        let mut state = NormState::new();
        let mut out = String::new();
        // Feed a starter (CCC=0)
        state.feed_entry('A', 0, &mut out, false);
        assert_eq!(state.current_starter, Some('A'));
        assert!(state.ccc_buf.is_empty());
        assert!(out.is_empty()); // No flush yet
    }

    #[test]
    fn feed_entry_combining_mark_buffers_in_ccc_buf() {
        let mut state = NormState::new();
        let mut out = String::new();
        // Set up a starter first
        state.feed_entry('e', 0, &mut out, false);
        // Feed combining acute (CCC=230)
        state.feed_entry('\u{0301}', 230, &mut out, false);
        assert_eq!(state.current_starter, Some('e'));
        assert!(!state.ccc_buf.is_empty());
        assert_eq!(state.ccc_buf.len(), 1);
        assert_eq!(state.ccc_buf.as_slice()[0].ch, '\u{0301}');
        assert_eq!(state.ccc_buf.as_slice()[0].ccc, 230);
    }

    #[test]
    fn feed_entry_two_starters_first_gets_flushed() {
        let mut state = NormState::new();
        let mut out = String::new();
        // Feed first starter
        state.feed_entry('A', 0, &mut out, false);
        assert!(out.is_empty());
        // Feed second starter -- first should be flushed to `out`
        state.feed_entry('B', 0, &mut out, false);
        assert_eq!(out, "A");
        assert_eq!(state.current_starter, Some('B'));
    }

    #[test]
    fn feed_entry_starter_to_starter_composition_hangul_lv() {
        let mut state = NormState::new();
        let mut out = String::new();
        // Hangul L
        state.feed_entry('\u{1100}', 0, &mut out, true);
        // Hangul V -- should compose with L in compose mode
        state.feed_entry('\u{1161}', 0, &mut out, true);
        // The composed syllable should be the current starter
        assert_eq!(state.current_starter, Some('\u{AC00}'));
        // Nothing flushed yet
        assert!(out.is_empty());
    }

    #[test]
    fn feed_entry_starter_to_starter_composition_e_acute() {
        let mut state = NormState::new();
        let mut out = String::new();
        // In compose mode, 'e' followed by combining acute (CCC=230)
        // is not starter-to-starter, but let's test the compose path
        // with a combining mark that composes.
        state.feed_entry('e', 0, &mut out, true);
        state.feed_entry('\u{0301}', 230, &mut out, true);
        // Now flush to get the composed result
        state.flush(&mut out, true);
        assert_eq!(out, "\u{00E9}"); // e-acute
    }

    #[test]
    fn feed_entry_nfd_starters_and_combining_marks() {
        let mut state = NormState::new();
        let mut out = String::new();
        // Feed starter
        state.feed_entry_nfd('A', 0, &mut out);
        assert_eq!(state.current_starter, Some('A'));
        // Feed combining grave (CCC=230)
        state.feed_entry_nfd('\u{0300}', 230, &mut out);
        assert_eq!(state.ccc_buf.len(), 1);
        // Feed new starter -- flushes A + combining grave
        state.feed_entry_nfd('B', 0, &mut out);
        assert_eq!(out, "A\u{0300}");
        assert_eq!(state.current_starter, Some('B'));
    }

    // ===================================================================
    // 3. NormState flush() and flush_nfd()
    // ===================================================================

    #[test]
    fn flush_no_starter_no_marks_nothing_emitted() {
        let mut state = NormState::new();
        let mut out = String::new();
        state.flush(&mut out, false);
        assert!(out.is_empty());
        state.flush(&mut out, true);
        assert!(out.is_empty());
    }

    #[test]
    fn flush_starter_only_emits_starter() {
        let mut state = NormState::new();
        let mut out = String::new();
        state.current_starter = Some('X');
        state.flush(&mut out, false);
        assert_eq!(out, "X");
    }

    #[test]
    fn flush_starter_one_combining_mark_no_compose() {
        let mut state = NormState::new();
        let mut out = String::new();
        state.current_starter = Some('e');
        state.ccc_buf.push('\u{0301}', 230); // combining acute
        state.flush(&mut out, false);
        assert_eq!(out, "e\u{0301}");
    }

    #[test]
    fn flush_starter_one_combining_mark_with_compose() {
        let mut state = NormState::new();
        let mut out = String::new();
        state.current_starter = Some('e');
        state.ccc_buf.push('\u{0301}', 230); // combining acute
        state.flush(&mut out, true);
        assert_eq!(out, "\u{00E9}"); // e-acute composed
    }

    #[test]
    fn flush_starter_multiple_ccc_disordered_marks_emits_sorted() {
        let mut state = NormState::new();
        let mut out = String::new();
        state.current_starter = Some('a');
        // Push marks in wrong CCC order: 230, 220, 202
        state.ccc_buf.push('\u{0301}', 230); // combining acute, CCC=230
        state.ccc_buf.push('\u{0323}', 220); // combining dot below, CCC=220
        state.ccc_buf.push('\u{0327}', 202); // combining cedilla, CCC=202
        state.flush(&mut out, false);
        // Should emit starter + marks sorted by CCC: 202, 220, 230
        let chars: Vec<char> = out.chars().collect();
        assert_eq!(chars[0], 'a');
        assert_eq!(chars[1], '\u{0327}'); // CCC=202
        assert_eq!(chars[2], '\u{0323}'); // CCC=220
        assert_eq!(chars[3], '\u{0301}'); // CCC=230
    }

    #[test]
    fn flush_orphan_combining_marks_no_starter_emits_sorted() {
        let mut state = NormState::new();
        let mut out = String::new();
        // No starter set, just orphan combining marks
        state.ccc_buf.push('\u{0301}', 230); // CCC=230
        state.ccc_buf.push('\u{0327}', 202); // CCC=202
        state.flush(&mut out, false);
        let chars: Vec<char> = out.chars().collect();
        assert_eq!(chars.len(), 2);
        assert_eq!(chars[0], '\u{0327}'); // CCC=202 first
        assert_eq!(chars[1], '\u{0301}'); // CCC=230 second
    }

    #[test]
    fn flush_nfd_no_starter_no_marks_nothing_emitted() {
        let mut state = NormState::new();
        let mut out = String::new();
        state.flush_nfd(&mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn flush_nfd_starter_only_emits_starter() {
        let mut state = NormState::new();
        let mut out = String::new();
        state.current_starter = Some('Z');
        state.flush_nfd(&mut out);
        assert_eq!(out, "Z");
    }

    #[test]
    fn flush_nfd_single_mark_fast_path_take_single_inline() {
        let mut state = NormState::new();
        let mut out = String::new();
        state.current_starter = Some('e');
        state.ccc_buf.push('\u{0301}', 230); // single combining mark
        // This should hit the take_single_inline fast path in flush_nfd
        state.flush_nfd(&mut out);
        assert_eq!(out, "e\u{0301}");
        // Buffer should be cleared
        assert!(state.ccc_buf.is_empty());
    }

    #[test]
    fn flush_nfd_multiple_marks_sorted() {
        let mut state = NormState::new();
        let mut out = String::new();
        state.current_starter = Some('o');
        state.ccc_buf.push('\u{0301}', 230); // CCC=230
        state.ccc_buf.push('\u{0327}', 202); // CCC=202
        state.flush_nfd(&mut out);
        let chars: Vec<char> = out.chars().collect();
        assert_eq!(chars[0], 'o');
        assert_eq!(chars[1], '\u{0327}'); // CCC=202
        assert_eq!(chars[2], '\u{0301}'); // CCC=230
    }

    #[test]
    fn flush_nfd_orphan_combining_marks_no_starter() {
        let mut state = NormState::new();
        let mut out = String::new();
        state.ccc_buf.push('\u{0301}', 230);
        state.ccc_buf.push('\u{0323}', 220);
        state.flush_nfd(&mut out);
        let chars: Vec<char> = out.chars().collect();
        assert_eq!(chars.len(), 2);
        assert_eq!(chars[0], '\u{0323}'); // CCC=220
        assert_eq!(chars[1], '\u{0301}'); // CCC=230
    }

    // ===================================================================
    // 4. normalize_impl() Cow::Borrowed path
    // ===================================================================

    #[test]
    fn normalize_impl_nfc_already_normalized_returns_borrowed() {
        // U+00C5 (A with ring) followed by U+0300 (combining grave).
        // This is already in NFC -- the quick check should return Maybe
        // (because U+0300 has NFC_QC=Maybe), but after normalization,
        // the output equals input, so Cow::Borrowed is returned.
        let input = "\u{00C5}\u{0300}";
        let result = normalize_impl(input, Form::Nfc);
        assert!(
            matches!(result, Cow::Borrowed(_)),
            "Expected Cow::Borrowed for already-NFC input with Maybe QC, got Cow::Owned({:?})",
            result
        );
        assert_eq!(&*result, input);
    }

    #[test]
    fn normalize_impl_nfc_maybe_borrowed_simd_path() {
        // Exercise the SIMD normalize_impl Maybe->Borrowed code path (line 720-721).
        // Input must be >= 64 bytes and trigger QC=Maybe but produce identical output.
        // 60 bytes of ASCII padding + "\u{00C5}\u{0300}" (already NFC, QC=Maybe).
        let mut input = String::new();
        input.push_str(&"a".repeat(60));
        input.push_str("\u{00C5}\u{0300}"); // Å + combining grave, already NFC
        assert!(input.len() >= 64, "input must be >= 64 bytes for SIMD path");
        let result = normalize_impl(&input, Form::Nfc);
        assert!(
            matches!(result, Cow::Borrowed(_)),
            "Expected Cow::Borrowed for >=64 byte already-NFC input with Maybe QC, got Cow::Owned({:?})",
            result
        );
        assert_eq!(&*result, &*input);
    }

    #[test]
    fn normalize_impl_ascii_returns_borrowed() {
        let input = "Hello, world!";
        let result = normalize_impl(input, Form::Nfc);
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(&*result, input);
    }

    #[test]
    fn normalize_impl_nfd_already_decomposed_returns_borrowed() {
        // "e" + combining acute is already NFD
        let input = "e\u{0301}";
        let result = normalize_impl(input, Form::Nfd);
        assert!(
            matches!(result, Cow::Borrowed(_)),
            "Expected Cow::Borrowed for already-NFD input"
        );
    }

    #[test]
    fn normalize_impl_nfc_not_normalized_returns_owned() {
        // NFD form of e-acute: "e" + combining acute -- not NFC
        let input = "e\u{0301}";
        let result = normalize_impl(input, Form::Nfc);
        assert!(matches!(result, Cow::Owned(_)));
        assert_eq!(&*result, "\u{00E9}");
    }

    // ===================================================================
    // 5. is_cjk_unified() boundary tests
    // ===================================================================

    #[test]
    fn cjk_unified_extension_a_start() {
        assert!(is_cjk_unified(0x3400));
    }

    #[test]
    fn cjk_unified_extension_a_end() {
        assert!(is_cjk_unified(0x4DBF));
    }

    #[test]
    fn cjk_unified_main_start() {
        assert!(is_cjk_unified(0x4E00));
    }

    #[test]
    fn cjk_unified_main_end() {
        assert!(is_cjk_unified(0x9FFF));
    }

    #[test]
    fn cjk_unified_just_before_extension_a() {
        assert!(!is_cjk_unified(0x33FF));
    }

    #[test]
    fn cjk_unified_gap_between_extension_a_and_main() {
        assert!(!is_cjk_unified(0x4DC0));
    }

    #[test]
    fn cjk_unified_just_after_main() {
        assert!(!is_cjk_unified(0xA000));
    }
}
