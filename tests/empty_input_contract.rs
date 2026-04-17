//! Consolidated empty-input contract tests.
//!
//! Closes the gap identified in
//! `docs/superpowers/specs/2026-04-17-full-edge-case-coverage-design.md`
//! section 4: every public entry point is called on `""` and must return
//! the expected empty-equivalent value.

use simd_normalizer::{
    CaseFoldMode, IsNormalized, NfcNormalizer, NfdNormalizer, NfkcNormalizer, NfkdNormalizer,
    UnicodeNormalization, are_confusable, casefold, nfc, nfd, nfkc, nfkd, skeleton,
};
use simd_normalizer::matching::{
    MatchingOptions, matches_normalized, normalize_for_matching, normalize_for_matching_utf16,
};
use std::borrow::Cow;

// ---------------------------------------------------------------------------
// UnicodeNormalization trait on ""
// ---------------------------------------------------------------------------

#[test]
fn empty_trait_nfc_returns_borrowed_empty() {
    let result = "".nfc();
    assert_eq!(&*result, "");
    assert!(matches!(result, Cow::Borrowed(_)));
}

#[test]
fn empty_trait_nfd_returns_borrowed_empty() {
    let result = "".nfd();
    assert_eq!(&*result, "");
    assert!(matches!(result, Cow::Borrowed(_)));
}

#[test]
fn empty_trait_nfkc_returns_borrowed_empty() {
    let result = "".nfkc();
    assert_eq!(&*result, "");
    assert!(matches!(result, Cow::Borrowed(_)));
}

#[test]
fn empty_trait_nfkd_returns_borrowed_empty() {
    let result = "".nfkd();
    assert_eq!(&*result, "");
    assert!(matches!(result, Cow::Borrowed(_)));
}

#[test]
fn empty_trait_is_normalized_all_true() {
    assert!("".is_nfc());
    assert!("".is_nfd());
    assert!("".is_nfkc());
    assert!("".is_nfkd());
}

// ---------------------------------------------------------------------------
// Free functions / normalizer methods on ""
// ---------------------------------------------------------------------------

#[test]
fn empty_free_fn_normalize_matches_trait() {
    assert_eq!(&*nfc().normalize(""), "");
    assert_eq!(&*nfd().normalize(""), "");
    assert_eq!(&*nfkc().normalize(""), "");
    assert_eq!(&*nfkd().normalize(""), "");
}

#[test]
fn empty_quick_check_all_forms_yes() {
    assert_eq!(NfcNormalizer::new().quick_check(""), IsNormalized::Yes);
    assert_eq!(NfdNormalizer::new().quick_check(""), IsNormalized::Yes);
    assert_eq!(NfkcNormalizer::new().quick_check(""), IsNormalized::Yes);
    assert_eq!(NfkdNormalizer::new().quick_check(""), IsNormalized::Yes);
}

#[test]
fn empty_normalize_to_returns_true_buf_unchanged() {
    let mut buf = String::from("keep-me");
    assert!(NfcNormalizer::new().normalize_to("", &mut buf));
    assert!(NfdNormalizer::new().normalize_to("", &mut buf));
    assert!(NfkcNormalizer::new().normalize_to("", &mut buf));
    assert!(NfkdNormalizer::new().normalize_to("", &mut buf));
    assert_eq!(buf, "keep-me");
}

// ---------------------------------------------------------------------------
// Casefold / skeleton / confusable on ""
// ---------------------------------------------------------------------------

#[test]
fn empty_casefold_standard_returns_borrowed_empty() {
    let result = casefold("", CaseFoldMode::Standard);
    assert_eq!(&*result, "");
    assert!(matches!(result, Cow::Borrowed(_)));
}

#[test]
fn empty_casefold_turkish_returns_borrowed_empty() {
    let result = casefold("", CaseFoldMode::Turkish);
    assert_eq!(&*result, "");
    assert!(matches!(result, Cow::Borrowed(_)));
}

// `casefold_char` is intentionally not tested for empty input: it takes a
// `char` (always present) not a `&str`, so "empty input" is not applicable.
// Per spec section 4, line 88.

#[test]
fn empty_skeleton_returns_empty_string() {
    assert_eq!(skeleton(""), "");
}

#[test]
fn empty_are_confusable_both_empty_is_true() {
    assert!(are_confusable("", ""));
}

// ---------------------------------------------------------------------------
// Matching pipeline on ""
// ---------------------------------------------------------------------------

#[test]
fn empty_normalize_for_matching_returns_empty_string() {
    assert_eq!(normalize_for_matching("", &MatchingOptions::default()), "");
}

#[test]
fn empty_normalize_for_matching_utf16_returns_empty_vec() {
    assert!(normalize_for_matching_utf16("", &MatchingOptions::default()).is_empty());
}

#[test]
fn empty_matches_normalized_both_empty_is_true() {
    assert!(matches_normalized("", "", &MatchingOptions::default()));
}

#[test]
fn empty_matches_normalized_left_empty_right_non_empty_is_false() {
    assert!(!matches_normalized("", "x", &MatchingOptions::default()));
}

#[test]
fn empty_matches_normalized_left_non_empty_right_empty_is_false() {
    assert!(!matches_normalized("x", "", &MatchingOptions::default()));
}
