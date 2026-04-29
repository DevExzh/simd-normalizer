// src/quick_check.rs

//! Quick-check for normalization forms (UAX#15 Section 9).
//!
//! Uses SIMD scanning to skip safe chunks in bulk for inputs >= 64 bytes.
//! Form-specific SIMD bounds and code-point range fast paths avoid trie
//! lookups for the vast majority of BMP characters.

use crate::simd;
use crate::tables;
use crate::utf8;

/// Result of a quick-check test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsNormalized {
    /// The string is definitely in the target normalization form.
    Yes,
    /// The string is definitely *not* in the target normalization form.
    No,
    /// The string *might* not be normalized; a full check is required.
    Maybe,
}

/// Convert a QC trie value (0=Y, 1=M, 2=N) to IsNormalized.
#[inline]
fn qc_value_to_result(v: u8) -> IsNormalized {
    match v {
        0 => IsNormalized::Yes,
        1 => IsNormalized::Maybe,
        _ => IsNormalized::No,
    }
}

/// Check if a code point is a CJK Unified Ideograph (CCC=0, QC=Yes for all forms).
#[inline(always)]
fn is_cjk_unified(cp: u32) -> bool {
    // BMP: CJK Unified Ideographs + Extension A (most common)
    (0x4E00..=0x9FFF).contains(&cp) || (0x3400..=0x4DBF).contains(&cp)
}

/// Check if a supplementary code point (cp >= 0x10000) is safe for all
/// normalization forms (CCC=0 and QC=Yes). Returns false only for narrow
/// exception ranges that may have decompositions or non-zero CCC.
#[inline(always)]
fn is_supp_safe(cp: u32) -> bool {
    if cp >= 0x20000 {
        // Plane 2+: safe except CJK Compatibility Ideographs Supplement
        return !(0x2F800..=0x2FA1F).contains(&cp);
    }
    // Plane 1: core emoji and symbols block (U+1F252-U+1FBEF) is safe.
    // Verified: no decompositions and CCC=0 for all normalization forms.
    (0x1F252..=0x1FBEF).contains(&cp)
}

/// Check if a code point is Hiragana or Katakana (CCC=0, QC=Yes for NFC/NFKC).
/// Excludes: combining marks U+3099-309A (CCC>0), NFKC-decomposing U+309B-309C,
/// U+309F (ゟ), U+30FF (ヿ).
#[inline(always)]
fn is_kana(cp: u32) -> bool {
    // Hiragana base (U+3041-3098)
    (0x3041..0x3099).contains(&cp)
        // Hiragana iteration marks (U+309D-309E)
        || cp == 0x309D
        || cp == 0x309E
        // Katakana (U+30A0-30FE)
        || (0x30A0..=0x30FE).contains(&cp)
}

/// Generic quick-check implementation.
///
/// For inputs >= 64 bytes, uses SIMD scanning to skip chunks in bulk.
/// For shorter inputs, falls back to a scalar character-by-character loop.
/// Returns as soon as a definitive No is found.
///
/// # Parameters
/// - `qc_shift`: bit shift to extract this form's 2-bit QC from the fused CCC+QC trie.
/// - `simd_bound`: SIMD scan threshold; bytes below this are skipped in bulk.
///   For NFC this is 0xCC (all chars below U+0300 are safe), for other forms 0xC0.
/// - `safe_below`: code point below which CCC=0 and QC=Yes is guaranteed.
/// - `hangul_safe`: whether Hangul Syllables (U+AC00..U+D7A3) are QC=Yes for this form.
/// - `kana_safe`: whether Hiragana/Katakana (U+3040..U+30FF) are QC=Yes for this form.
/// - `latin1_upper_safe`: whether U+00C0..U+00FF (precomposed Latin Supplement upper)
///   is uniformly CCC=0 and QC=Yes. True for NFC/NFKC (precomposed forms are kept
///   precomposed under both compose-normalizing forms). False for NFD/NFKD where
///   they all decompose. NFC already has `simd_bound=0xCC` which skips this range
///   in bulk; the flag is consulted on the bit-walk for forms whose `simd_bound`
///   does flag bytes in the U+00C0..U+00FF lead-byte range.
#[inline]
fn quick_check_impl(
    input: &str,
    qc_shift: u32,
    simd_bound: u8,
    safe_below: u32,
    hangul_safe: bool,
    kana_safe: bool,
    latin1_upper_safe: bool,
) -> IsNormalized {
    let bytes = input.as_bytes();
    let len = bytes.len();

    if len < 64 {
        return quick_check_scalar(
            input,
            qc_shift,
            safe_below,
            hangul_safe,
            kana_safe,
            latin1_upper_safe,
        );
    }

    let ptr = bytes.as_ptr();

    let mut last_ccc: u8 = 0;
    let mut result = IsNormalized::Yes;
    // Byte offset past the last character we've examined.
    let mut processed_up_to: usize = 0;
    let mut pos: usize = 0;

    // SIMD chunk loop: skip chunks where all bytes < simd_bound in bulk.
    while pos + 64 <= len {
        // SAFETY: pos + 64 <= len, so ptr.add(pos) is valid for 64 bytes.
        let mask = unsafe { simd::scan_chunk(ptr.add(pos), simd_bound) };
        let chunk_end = pos + 64;

        if mask == 0 {
            // All bytes < simd_bound — characters in this chunk are either ASCII
            // or known-safe non-ASCII (CCC=0, QC=Yes). CCC resets to 0.
            last_ccc = 0;
            processed_up_to = chunk_end;
            pos = chunk_end;
            continue;
        }

        // Walk set bits — each is a lead byte of a character that needs inspection.
        let chunk_start = pos;
        let mut chunk_mask = mask;
        while chunk_mask != 0 {
            let bit_pos = chunk_mask.trailing_zeros() as usize;
            chunk_mask &= chunk_mask.wrapping_sub(1); // clear lowest set bit

            let byte_pos = chunk_start + bit_pos;

            // Skip bytes already covered by a previous multi-byte decode.
            if byte_pos < processed_up_to {
                continue;
            }

            // Gap before this lead byte → safe characters → CCC resets to 0.
            if byte_pos > processed_up_to {
                last_ccc = 0;
            }

            // Decode the character at this position.
            let (ch, width) = utf8::decode_char_at(bytes, byte_pos);
            processed_up_to = byte_pos + width;

            // Fast path: known-safe code point ranges (CCC=0 and QC=Yes).
            let cp = ch as u32;
            if cp < safe_below
                || (latin1_upper_safe && (0x00C0..0x0100).contains(&cp))
                || is_cjk_unified(cp)
                || (hangul_safe && (0xAC00..=0xD7A3).contains(&cp))
                || (kana_safe && is_kana(cp))
                || (cp >= 0x10000 && is_supp_safe(cp))
            {
                last_ccc = 0;
                continue;
            }

            // Fused CCC + QC lookup (single trie access).
            let (ccc, qc) = tables::lookup_ccc_qc(ch, qc_shift);
            if ccc != 0 && last_ccc > ccc {
                return IsNormalized::No;
            }

            // Check QC property.
            match qc_value_to_result(qc) {
                IsNormalized::No => return IsNormalized::No,
                IsNormalized::Maybe => result = IsNormalized::Maybe,
                IsNormalized::Yes => {},
            }

            last_ccc = ccc;
        }

        // Trailing safe bytes in this chunk after the last flagged char.
        if processed_up_to < chunk_end {
            last_ccc = 0;
            processed_up_to = chunk_end;
        }

        pos = chunk_end;
    }

    // Scalar tail for remaining bytes after the last full 64-byte chunk.
    let tail_start = processed_up_to.max(pos);
    if tail_start > processed_up_to {
        // Gap of safe characters between last processed char and tail start.
        last_ccc = 0;
    }
    let mut tail_pos = tail_start;
    while tail_pos < len {
        let b = bytes[tail_pos];
        if b < 0x80 {
            // ASCII: CCC=0, QC=Yes for all forms.
            last_ccc = 0;
            tail_pos += 1;
            continue;
        }
        // Skip continuation bytes from a character that crossed the chunk/tail
        // boundary. Its lead byte was < simd_bound, so it is safe (CCC=0, QC=Yes).
        if utf8::is_continuation_byte(b) {
            tail_pos += 1;
            continue;
        }
        // Lead byte of a non-ASCII character.
        let (ch, width) = utf8::decode_char_at(bytes, tail_pos);

        // Fast path: known-safe code point ranges.
        let cp = ch as u32;
        if cp < safe_below
            || (latin1_upper_safe && (0x00C0..0x0100).contains(&cp))
            || is_cjk_unified(cp)
            || (hangul_safe && (0xAC00..=0xD7A3).contains(&cp))
            || (cp >= 0x10000 && is_supp_safe(cp))
        {
            last_ccc = 0;
            tail_pos += width;
            continue;
        }

        let (ccc, qc) = tables::lookup_ccc_qc(ch, qc_shift);
        if ccc != 0 && last_ccc > ccc {
            return IsNormalized::No;
        }
        match qc_value_to_result(qc) {
            IsNormalized::No => return IsNormalized::No,
            IsNormalized::Maybe => result = IsNormalized::Maybe,
            IsNormalized::Yes => {},
        }
        last_ccc = ccc;
        tail_pos += width;
    }

    result
}

/// Scalar quick-check for short inputs (< 64 bytes).
#[inline]
fn quick_check_scalar(
    input: &str,
    qc_shift: u32,
    safe_below: u32,
    hangul_safe: bool,
    kana_safe: bool,
    latin1_upper_safe: bool,
) -> IsNormalized {
    let mut last_ccc: u8 = 0;
    let mut result = IsNormalized::Yes;

    for ch in input.chars() {
        let cp = ch as u32;

        // ASCII fast path
        if cp <= 0x7F {
            last_ccc = 0;
            continue;
        }

        // Fast path: known-safe code point ranges (CCC=0 and QC=Yes).
        if cp < safe_below
            || (latin1_upper_safe && (0x00C0..0x0100).contains(&cp))
            || is_cjk_unified(cp)
            || (hangul_safe && (0xAC00..=0xD7A3).contains(&cp))
            || (kana_safe && is_kana(cp))
            || (cp >= 0x10000 && is_supp_safe(cp))
        {
            last_ccc = 0;
            continue;
        }

        let (ccc, qc) = tables::lookup_ccc_qc(ch, qc_shift);

        // CCC must be non-decreasing among non-zero values.
        if ccc != 0 && last_ccc > ccc {
            return IsNormalized::No;
        }

        match qc_value_to_result(qc) {
            IsNormalized::No => return IsNormalized::No,
            IsNormalized::Maybe => result = IsNormalized::Maybe,
            IsNormalized::Yes => {},
        }

        last_ccc = ccc;
    }

    result
}

// ---------------------------------------------------------------------------
// SIMD bound and safe-below thresholds by normalization form
// ---------------------------------------------------------------------------
//
// NFC:  simd_bound=0xCC, safe_below=0x0300, hangul_safe=true, kana_safe=true
//       All chars U+0000..U+02FF have CCC=0 and NFC_QC=Yes.
//       The first CCC != 0 is U+0300 (lead byte 0xCC).
//       CJK Unified, Hangul Syllables, and Hiragana/Katakana are NFC-safe.
//
// NFD:  simd_bound=0xC3, safe_below=0x00C0, hangul_safe=false, kana_safe=false
//       U+00C0 is first NFD_QC=No (lead byte 0xC3).
//       Hangul Syllables and some kana have NFD_QC=No (they decompose).
//
// NFKC: simd_bound=0xC0, safe_below=0x00A0, hangul_safe=true, kana_safe=true
//       U+00A0 is first NFKC_QC=No (NBSP → SPACE).
//       Kana are NFKC-safe (only halfwidth/enclosed forms decompose, in other blocks).
//
// NFKD: simd_bound=0xC0, safe_below=0x00A0, hangul_safe=false, kana_safe=false
//       Same as NFKC threshold, but Hangul and some kana decompose.

/// Quick-check whether `input` is in NFC.
#[cfg(not(feature = "quick_check_oracle"))]
pub(crate) fn quick_check_nfc(input: &str) -> IsNormalized {
    quick_check_impl(
        input,
        tables::CCC_QC_NFC_SHIFT,
        0xCC,
        0x0300,
        true,
        true,
        true,
    )
}

/// Quick-check whether `input` is in NFC.
#[cfg(feature = "quick_check_oracle")]
pub fn quick_check_nfc(input: &str) -> IsNormalized {
    quick_check_impl(
        input,
        tables::CCC_QC_NFC_SHIFT,
        0xCC,
        0x0300,
        true,
        true,
        true,
    )
}

/// Quick-check whether `input` is in NFD.
#[cfg(not(feature = "quick_check_oracle"))]
pub(crate) fn quick_check_nfd(input: &str) -> IsNormalized {
    quick_check_impl(
        input,
        tables::CCC_QC_NFD_SHIFT,
        0xC3,
        0x00C0,
        false,
        false,
        false,
    )
}

/// Quick-check whether `input` is in NFD.
#[cfg(feature = "quick_check_oracle")]
pub fn quick_check_nfd(input: &str) -> IsNormalized {
    quick_check_impl(
        input,
        tables::CCC_QC_NFD_SHIFT,
        0xC3,
        0x00C0,
        false,
        false,
        false,
    )
}

/// Quick-check whether `input` is in NFKC.
#[cfg(not(feature = "quick_check_oracle"))]
pub(crate) fn quick_check_nfkc(input: &str) -> IsNormalized {
    quick_check_impl(
        input,
        tables::CCC_QC_NFKC_SHIFT,
        0xC0,
        0x00A0,
        true,
        true,
        true,
    )
}

/// Quick-check whether `input` is in NFKC.
#[cfg(feature = "quick_check_oracle")]
pub fn quick_check_nfkc(input: &str) -> IsNormalized {
    quick_check_impl(
        input,
        tables::CCC_QC_NFKC_SHIFT,
        0xC0,
        0x00A0,
        true,
        true,
        true,
    )
}

/// Quick-check whether `input` is in NFKD.
#[cfg(not(feature = "quick_check_oracle"))]
pub(crate) fn quick_check_nfkd(input: &str) -> IsNormalized {
    quick_check_impl(
        input,
        tables::CCC_QC_NFKD_SHIFT,
        0xC0,
        0x00A0,
        false,
        false,
        false,
    )
}

/// Quick-check whether `input` is in NFKD.
#[cfg(feature = "quick_check_oracle")]
pub fn quick_check_nfkd(input: &str) -> IsNormalized {
    quick_check_impl(
        input,
        tables::CCC_QC_NFKD_SHIFT,
        0xC0,
        0x00A0,
        false,
        false,
        false,
    )
}

// ---------------------------------------------------------------------------
// Oracle (slow-path) implementation for differential testing.
// ---------------------------------------------------------------------------
//
// The oracle deliberately routes every flagged byte through the Layer-2
// decode + range + trie path, i.e. it calls `simd::scan_chunk` (no
// safe_lead_mask) and omits the short-circuit. It exists so that
// tests/quick_check_fastpath_equivalence.rs can assert
// `quick_check_X(s) == quick_check_X_oracle(s)` for every form, 8192
// cases per form, on arbitrary Unicode input.

#[cfg(feature = "quick_check_oracle")]
#[inline]
fn quick_check_impl_oracle(
    input: &str,
    qc_shift: u32,
    simd_bound: u8,
    safe_below: u32,
    hangul_safe: bool,
    kana_safe: bool,
) -> IsNormalized {
    let bytes = input.as_bytes();
    let len = bytes.len();

    if len < 64 {
        return quick_check_scalar(input, qc_shift, safe_below, hangul_safe, kana_safe, false);
    }

    let ptr = bytes.as_ptr();
    let mut last_ccc: u8 = 0;
    let mut result = IsNormalized::Yes;
    let mut processed_up_to: usize = 0;
    let mut pos: usize = 0;

    while pos + 64 <= len {
        // SAFETY: pos + 64 <= len, so ptr.add(pos) is valid for 64 bytes.
        let mask = unsafe { simd::scan_chunk(ptr.add(pos), simd_bound) };
        let chunk_end = pos + 64;

        if mask == 0 {
            last_ccc = 0;
            processed_up_to = chunk_end;
            pos = chunk_end;
            continue;
        }

        let chunk_start = pos;
        let mut chunk_mask = mask;
        while chunk_mask != 0 {
            let bit_pos = chunk_mask.trailing_zeros() as usize;
            chunk_mask &= chunk_mask.wrapping_sub(1);

            let byte_pos = chunk_start + bit_pos;
            if byte_pos < processed_up_to {
                continue;
            }
            if byte_pos > processed_up_to {
                last_ccc = 0;
            }

            let (ch, width) = utf8::decode_char_at(bytes, byte_pos);
            processed_up_to = byte_pos + width;

            let cp = ch as u32;
            if cp < safe_below
                || is_cjk_unified(cp)
                || (hangul_safe && (0xAC00..=0xD7A3).contains(&cp))
                || (kana_safe && is_kana(cp))
                || (cp >= 0x10000 && is_supp_safe(cp))
            {
                last_ccc = 0;
                continue;
            }

            let (ccc, qc) = tables::lookup_ccc_qc(ch, qc_shift);
            if ccc != 0 && last_ccc > ccc {
                return IsNormalized::No;
            }
            match qc_value_to_result(qc) {
                IsNormalized::No => return IsNormalized::No,
                IsNormalized::Maybe => result = IsNormalized::Maybe,
                IsNormalized::Yes => {},
            }
            last_ccc = ccc;
        }

        if processed_up_to < chunk_end {
            last_ccc = 0;
            processed_up_to = chunk_end;
        }
        pos = chunk_end;
    }

    // Scalar tail (identical to quick_check_impl; duplicated verbatim so
    // the oracle stays a single self-contained function).
    let tail_start = processed_up_to.max(pos);
    if tail_start > processed_up_to {
        last_ccc = 0;
    }
    let mut tail_pos = tail_start;
    while tail_pos < len {
        let b = bytes[tail_pos];
        if b < 0x80 {
            last_ccc = 0;
            tail_pos += 1;
            continue;
        }
        if utf8::is_continuation_byte(b) {
            tail_pos += 1;
            continue;
        }
        let (ch, width) = utf8::decode_char_at(bytes, tail_pos);
        let cp = ch as u32;
        if cp < safe_below
            || is_cjk_unified(cp)
            || (hangul_safe && (0xAC00..=0xD7A3).contains(&cp))
            || (cp >= 0x10000 && is_supp_safe(cp))
        {
            last_ccc = 0;
            tail_pos += width;
            continue;
        }
        let (ccc, qc) = tables::lookup_ccc_qc(ch, qc_shift);
        if ccc != 0 && last_ccc > ccc {
            return IsNormalized::No;
        }
        match qc_value_to_result(qc) {
            IsNormalized::No => return IsNormalized::No,
            IsNormalized::Maybe => result = IsNormalized::Maybe,
            IsNormalized::Yes => {},
        }
        last_ccc = ccc;
        tail_pos += width;
    }

    result
}

/// Oracle NFC quick-check. Differential-testing only.
#[cfg(feature = "quick_check_oracle")]
pub fn quick_check_nfc_oracle(input: &str) -> IsNormalized {
    quick_check_impl_oracle(input, tables::CCC_QC_NFC_SHIFT, 0xCC, 0x0300, true, true)
}

/// Oracle NFD quick-check. Differential-testing only.
#[cfg(feature = "quick_check_oracle")]
pub fn quick_check_nfd_oracle(input: &str) -> IsNormalized {
    quick_check_impl_oracle(input, tables::CCC_QC_NFD_SHIFT, 0xC3, 0x00C0, false, false)
}

/// Oracle NFKC quick-check. Differential-testing only.
#[cfg(feature = "quick_check_oracle")]
pub fn quick_check_nfkc_oracle(input: &str) -> IsNormalized {
    quick_check_impl_oracle(input, tables::CCC_QC_NFKC_SHIFT, 0xC0, 0x00A0, true, true)
}

/// Oracle NFKD quick-check. Differential-testing only.
#[cfg(feature = "quick_check_oracle")]
pub fn quick_check_nfkd_oracle(input: &str) -> IsNormalized {
    quick_check_impl_oracle(input, tables::CCC_QC_NFKD_SHIFT, 0xC0, 0x00A0, false, false)
}

// ---------------------------------------------------------------------------
// Definitive is_normalized checks (resolve Maybe via full normalization)
// ---------------------------------------------------------------------------
//
// These delegate to the main normalizer for the Maybe case, ensuring the
// quick-check resolution uses the same code path as actual normalization.

/// Definitive NFC check.
pub(crate) fn is_normalized_nfc(input: &str) -> bool {
    match quick_check_nfc(input) {
        IsNormalized::Yes => true,
        IsNormalized::No => false,
        IsNormalized::Maybe => &*crate::nfc().normalize(input) == input,
    }
}

/// Definitive NFD check.
pub(crate) fn is_normalized_nfd(input: &str) -> bool {
    match quick_check_nfd(input) {
        IsNormalized::Yes => true,
        IsNormalized::No => false,
        IsNormalized::Maybe => &*crate::nfd().normalize(input) == input,
    }
}

/// Definitive NFKC check.
pub(crate) fn is_normalized_nfkc(input: &str) -> bool {
    match quick_check_nfkc(input) {
        IsNormalized::Yes => true,
        IsNormalized::No => false,
        IsNormalized::Maybe => &*crate::nfkc().normalize(input) == input,
    }
}

/// Definitive NFKD check.
pub(crate) fn is_normalized_nfkd(input: &str) -> bool {
    match quick_check_nfkd(input) {
        IsNormalized::Yes => true,
        IsNormalized::No => false,
        IsNormalized::Maybe => &*crate::nfkd().normalize(input) == input,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;
    use alloc::string::String;

    // ---- ASCII fast path ----

    #[test]
    fn ascii_is_nfc() {
        assert_eq!(quick_check_nfc("Hello, world!"), IsNormalized::Yes);
    }

    #[test]
    fn ascii_is_nfd() {
        assert_eq!(quick_check_nfd("Hello, world!"), IsNormalized::Yes);
    }

    #[test]
    fn ascii_is_nfkc() {
        assert_eq!(quick_check_nfkc("Hello, world!"), IsNormalized::Yes);
    }

    #[test]
    fn ascii_is_nfkd() {
        assert_eq!(quick_check_nfkd("Hello, world!"), IsNormalized::Yes);
    }

    #[test]
    fn empty_string_is_normalized() {
        assert_eq!(quick_check_nfc(""), IsNormalized::Yes);
        assert_eq!(quick_check_nfd(""), IsNormalized::Yes);
        assert_eq!(quick_check_nfkc(""), IsNormalized::Yes);
        assert_eq!(quick_check_nfkd(""), IsNormalized::Yes);
    }

    // ---- NFC checks ----

    #[test]
    fn precomposed_is_nfc_yes() {
        assert_eq!(quick_check_nfc("\u{00E9}"), IsNormalized::Yes);
    }

    #[test]
    fn decomposed_is_not_nfc() {
        let nfd = "e\u{0301}";
        let result = quick_check_nfc(nfd);
        assert!(
            result == IsNormalized::No || result == IsNormalized::Maybe,
            "NFD form must not be Yes for NFC, got {:?}",
            result,
        );
    }

    // ---- NFD checks ----

    #[test]
    fn precomposed_is_not_nfd() {
        assert_eq!(quick_check_nfd("\u{00E9}"), IsNormalized::No);
    }

    // ---- CCC ordering ----

    #[test]
    fn wrong_ccc_order_is_no() {
        let bad_order = "a\u{0301}\u{0327}"; // acute(230) then cedilla(202)
        assert_eq!(quick_check_nfc(bad_order), IsNormalized::No);
        assert_eq!(quick_check_nfd(bad_order), IsNormalized::No);
    }

    #[test]
    fn correct_ccc_order_not_rejected() {
        // Use Hebrew accents which are NFC_QC=Yes but have non-zero CCC.
        // U+0591 HEBREW ACCENT ETNAHTA (CCC=220), U+05A1 HEBREW ACCENT PAZER (CCC=230)
        let good_order = "a\u{0591}\u{05A1}";
        let result = quick_check_nfc(good_order);
        assert_ne!(result, IsNormalized::No);
    }

    // ---- Range fast path tests ----

    #[test]
    fn latin1_supplement_is_nfc() {
        // U+00C0..U+00FF are all NFC_QC=Yes
        let latin1 = "\u{00C0}\u{00E9}\u{00F6}\u{00FC}\u{00FF}";
        assert_eq!(quick_check_nfc(latin1), IsNormalized::Yes);
    }

    #[test]
    fn latin_extended_is_nfc() {
        // U+0100..U+02FF are all NFC_QC=Yes
        let extended = "\u{0100}\u{017E}\u{0250}\u{02FF}";
        assert_eq!(quick_check_nfc(extended), IsNormalized::Yes);
    }

    #[test]
    fn cjk_is_nfc() {
        let cjk = "\u{4E00}\u{9FFF}\u{3400}\u{4DBF}";
        assert_eq!(quick_check_nfc(cjk), IsNormalized::Yes);
    }

    #[test]
    fn hangul_syllable_is_nfc() {
        let hangul = "\u{AC00}\u{D7A3}";
        assert_eq!(quick_check_nfc(hangul), IsNormalized::Yes);
    }

    #[test]
    fn hangul_syllable_is_not_nfd() {
        let hangul = "\u{AC00}";
        assert_eq!(quick_check_nfd(hangul), IsNormalized::No);
    }

    #[test]
    fn latin1_is_not_nfd() {
        // U+00C0 decomposes in NFD
        assert_eq!(quick_check_nfd("\u{00C0}"), IsNormalized::No);
    }

    #[test]
    fn nbsp_is_not_nfkc() {
        // U+00A0 (NBSP) → U+0020 (SPACE) in NFKC
        assert_eq!(quick_check_nfkc("\u{00A0}"), IsNormalized::No);
    }

    // ---- is_normalized definitive checks ----

    #[test]
    fn is_normalized_nfc_ascii() {
        assert!(is_normalized_nfc("Hello"));
    }

    #[test]
    fn is_normalized_nfc_precomposed() {
        assert!(is_normalized_nfc("\u{00E9}"));
    }

    #[test]
    fn is_normalized_nfd_decomposed() {
        assert!(is_normalized_nfd("e\u{0301}"));
    }

    #[test]
    fn is_normalized_nfc_rejects_nfd() {
        assert!(!is_normalized_nfc("e\u{0301}"));
    }

    #[test]
    fn is_normalized_nfd_rejects_nfc() {
        assert!(!is_normalized_nfd("\u{00E9}"));
    }

    #[test]
    fn safe_lead_interleaved_with_combining_marks_across_chunk() {
        // 128 bytes spanning two SIMD chunks.
        // Pattern: CJK ideograph (3 bytes, lead 0xE4..=0xE9, safe-lead) +
        //          'a' (1 byte, ASCII) +
        //          U+0591 HEBREW ACCENT ETNAHTA (CCC=220, NFC_QC=Yes, lead 0xD6 -> decode path).
        // The U+0591 must be observed after a safe-lead reset of last_ccc so that
        // the *next* non-zero CCC mark is accepted as non-decreasing.
        //
        // 16 repetitions of (CJK=3 + 'a'=1 + U+0591=2 + 'b'=1 + 'b'=1) = 16 * 8 = 128 bytes.
        let unit = "\u{4E2D}a\u{0591}bb";
        let s: String = unit.repeat(16);
        assert_eq!(s.len(), 128);
        // All code points are NFC_QC=Yes with monotonic (or zero) CCC, so NFC=Yes.
        assert_eq!(quick_check_nfc(&s), IsNormalized::Yes);
        // NFD: U+0591 is NFD_QC=Yes, CJK Unified is NFD_QC=Yes, ASCII is safe.
        assert_eq!(quick_check_nfd(&s), IsNormalized::Yes);
        assert_eq!(quick_check_nfkc(&s), IsNormalized::Yes);
        assert_eq!(quick_check_nfkd(&s), IsNormalized::Yes);
    }

    #[test]
    fn safe_lead_then_out_of_order_combining_is_no() {
        // Regression: if the safe-lead short-circuit fails to set last_ccc=0,
        // a subsequent same-position CCC check could mis-order. Build an input
        // where CJK (CCC=0 safe-lead) is followed by a correctly-ordered
        // combining sequence, then a mis-ordered one; expect No.
        // U+0301 ACUTE (CCC=230), U+0327 CEDILLA (CCC=202).
        let unit = "\u{4E2D}a\u{0301}\u{0327}"; // bad order after safe-lead + ASCII
        let padding = "x".repeat(64); // force >= 64-byte path
        let s = format!("{}{}", padding, unit);
        assert!(s.len() >= 64);
        assert_eq!(quick_check_nfc(&s), IsNormalized::No);
    }

    #[cfg(feature = "quick_check_oracle")]
    #[test]
    fn oracle_matches_fastpath_on_fixed_input() {
        let s = "\u{4E2D}a\u{0591}bb".repeat(16);
        assert_eq!(quick_check_nfc(&s), super::quick_check_nfc_oracle(&s));
        assert_eq!(quick_check_nfd(&s), super::quick_check_nfd_oracle(&s));
        assert_eq!(quick_check_nfkc(&s), super::quick_check_nfkc_oracle(&s));
        assert_eq!(quick_check_nfkd(&s), super::quick_check_nfkd_oracle(&s));
    }
}
