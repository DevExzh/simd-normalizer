// src/quick_check.rs

//! Quick-check for normalization forms (UAX#15 Section 9).

use crate::ccc::canonical_combining_class;
use crate::tables;

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

/// Generic quick-check implementation.
///
/// Walks the string character-by-character, checking the QC property and
/// tracking CCC ordering. Returns as soon as a definitive No is found.
fn quick_check_impl(input: &str, qc_lookup: fn(char) -> u8) -> IsNormalized {
    let mut last_ccc: u8 = 0;
    let mut result = IsNormalized::Yes;

    for ch in input.chars() {
        // ASCII fast path
        if (ch as u32) <= 0x7F {
            last_ccc = 0;
            continue;
        }

        let ccc = canonical_combining_class(ch);

        // CCC must be non-decreasing among non-zero values.
        if ccc != 0 && last_ccc > ccc {
            return IsNormalized::No;
        }

        match qc_value_to_result(qc_lookup(ch)) {
            IsNormalized::No => return IsNormalized::No,
            IsNormalized::Maybe => result = IsNormalized::Maybe,
            IsNormalized::Yes => {}
        }

        last_ccc = ccc;
    }

    result
}

/// Quick-check whether `input` is in NFC.
pub(crate) fn quick_check_nfc(input: &str) -> IsNormalized {
    quick_check_impl(input, tables::lookup_nfc_qc)
}

/// Quick-check whether `input` is in NFD.
pub(crate) fn quick_check_nfd(input: &str) -> IsNormalized {
    quick_check_impl(input, tables::lookup_nfd_qc)
}

/// Quick-check whether `input` is in NFKC.
pub(crate) fn quick_check_nfkc(input: &str) -> IsNormalized {
    quick_check_impl(input, tables::lookup_nfkc_qc)
}

/// Quick-check whether `input` is in NFKD.
pub(crate) fn quick_check_nfkd(input: &str) -> IsNormalized {
    quick_check_impl(input, tables::lookup_nfkd_qc)
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
}
