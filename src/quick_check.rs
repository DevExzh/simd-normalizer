// src/quick_check.rs

//! Quick-check for normalization forms (UAX#15 Section 9).

use alloc::string::String;
use alloc::vec::Vec;

use crate::ccc::{self, canonical_combining_class, CccBuffer};
use crate::compose;
use crate::decompose::{self, DecompForm};
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

/// Full decomposition + canonical ordering for a string.
fn normalize_decomposed(input: &str, form: DecompForm) -> String {
    let mut buf = CccBuffer::new();

    // Decompose the entire string into the buffer.
    for ch in input.chars() {
        decompose::decompose(ch, &mut buf, form);
    }

    // Collect all entries, then sort combining sequences by CCC.
    let all_entries: Vec<ccc::CharAndCcc> = buf.as_slice().to_vec();

    let mut result = String::with_capacity(input.len());
    let mut segment_start = 0;

    for i in 0..all_entries.len() {
        if all_entries[i].ccc == 0 && i > segment_start {
            // Output the segment [segment_start..i) with sorted combiners.
            output_sorted_segment(&all_entries[segment_start..i], &mut result);
            segment_start = i;
        }
    }
    // Output the final segment.
    if segment_start < all_entries.len() {
        output_sorted_segment(&all_entries[segment_start..], &mut result);
    }

    result
}

fn output_sorted_segment(segment: &[ccc::CharAndCcc], result: &mut String) {
    if segment.is_empty() {
        return;
    }

    // First entry is the starter (CCC=0).
    result.push(segment[0].ch);

    if segment.len() > 1 {
        // Sort combiners by CCC (stable).
        let mut combiners: Vec<ccc::CharAndCcc> = segment[1..].to_vec();
        combiners.sort_by_key(|e| e.ccc);

        for entry in &combiners {
            result.push(entry.ch);
        }
    }
}

/// Full NFC normalization for comparison.
fn normalize_nfc_string(input: &str) -> String {
    let nfd = normalize_decomposed(input, DecompForm::Canonical);
    compose_string(&nfd)
}

/// Full NFKC normalization for comparison.
fn normalize_nfkc_string(input: &str) -> String {
    let nfkd = normalize_decomposed(input, DecompForm::Compatible);
    compose_string(&nfkd)
}

/// Compose a decomposed + canonically ordered string.
fn compose_string(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut current_starter: Option<char> = None;
    let mut combining: Vec<ccc::CharAndCcc> = Vec::new();

    for ch in input.chars() {
        let ccc = canonical_combining_class(ch);

        if ccc == 0 {
            // New starter. Flush the previous sequence.
            if let Some(starter) = current_starter {
                flush_composed(starter, &combining, &mut result);
                combining.clear();
            }
            current_starter = Some(ch);
        } else if current_starter.is_none() {
            // Leading combining marks: no starter to compose with.
            result.push(ch);
        } else {
            combining.push(ccc::CharAndCcc { ch, ccc });
        }
    }

    // Flush final sequence.
    if let Some(starter) = current_starter {
        flush_composed(starter, &combining, &mut result);
    }

    result
}

/// Compose a starter with its combining sequence and append to result.
#[inline]
fn flush_composed(starter: char, combining: &[ccc::CharAndCcc], result: &mut String) {
    let (composed, remaining) = compose::compose_combining_sequence(starter, combining);
    result.push(composed);
    for &rem_ch in &remaining {
        result.push(rem_ch);
    }
}

/// Definitive NFC check.
pub(crate) fn is_normalized_nfc(input: &str) -> bool {
    match quick_check_nfc(input) {
        IsNormalized::Yes => true,
        IsNormalized::No => false,
        IsNormalized::Maybe => normalize_nfc_string(input) == input,
    }
}

/// Definitive NFD check.
pub(crate) fn is_normalized_nfd(input: &str) -> bool {
    match quick_check_nfd(input) {
        IsNormalized::Yes => true,
        IsNormalized::No => false,
        IsNormalized::Maybe => normalize_decomposed(input, DecompForm::Canonical) == input,
    }
}

/// Definitive NFKC check.
pub(crate) fn is_normalized_nfkc(input: &str) -> bool {
    match quick_check_nfkc(input) {
        IsNormalized::Yes => true,
        IsNormalized::No => false,
        IsNormalized::Maybe => normalize_nfkc_string(input) == input,
    }
}

/// Definitive NFKD check.
pub(crate) fn is_normalized_nfkd(input: &str) -> bool {
    match quick_check_nfkd(input) {
        IsNormalized::Yes => true,
        IsNormalized::No => false,
        IsNormalized::Maybe => normalize_decomposed(input, DecompForm::Compatible) == input,
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
