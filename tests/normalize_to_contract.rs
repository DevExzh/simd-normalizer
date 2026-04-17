//! Contract tests for `NfxNormalizer::normalize_to`.
//!
//! Closes the gap identified in
//! `docs/superpowers/specs/2026-04-17-full-edge-case-coverage-design.md`
//! section 1: append-into-empty / non-empty buffer, return-value semantics,
//! repeated-call concatenation, already-normalized vs needs-normalization,
//! empty input, and larger-input growth.

use simd_normalizer::{NfcNormalizer, NfdNormalizer, NfkcNormalizer, NfkdNormalizer};
use std::borrow::Cow;
// ---------------------------------------------------------------------------
// Append into empty buffer
// ---------------------------------------------------------------------------

#[test]
fn nfc_normalize_to_into_empty_matches_normalize() {
    let n = NfcNormalizer::new();
    let input = "A\u{030A}";
    let mut buf = String::new();
    let already = n.normalize_to(input, &mut buf);
    assert_eq!(buf, &*n.normalize(input));
    assert_eq!(already, matches!(n.normalize(input), Cow::Borrowed(_)));
    assert!(!already, "A+ring should not already be NFC");
}

#[test]
fn nfd_normalize_to_into_empty_matches_normalize() {
    let n = NfdNormalizer::new();
    let input = "\u{00C5}"; // Å (precomposed) — NFD decomposes to A+ring
    let mut buf = String::new();
    let already = n.normalize_to(input, &mut buf);
    assert_eq!(buf, &*n.normalize(input));
    assert_eq!(already, matches!(n.normalize(input), Cow::Borrowed(_)));
    assert!(!already, "precomposed Å should not already be NFD");
}

#[test]
fn nfkc_normalize_to_into_empty_matches_normalize() {
    let n = NfkcNormalizer::new();
    let input = "\u{FB01}"; // ﬁ ligature — NFKC decomposes then composes to "fi"
    let mut buf = String::new();
    let already = n.normalize_to(input, &mut buf);
    assert_eq!(buf, &*n.normalize(input));
    assert_eq!(already, matches!(n.normalize(input), Cow::Borrowed(_)));
    assert!(!already, "fi ligature should not already be NFKC");
}

#[test]
fn nfkd_normalize_to_into_empty_matches_normalize() {
    let n = NfkdNormalizer::new();
    let input = "\u{FB01}";
    let mut buf = String::new();
    let already = n.normalize_to(input, &mut buf);
    assert_eq!(buf, &*n.normalize(input));
    assert_eq!(already, matches!(n.normalize(input), Cow::Borrowed(_)));
    assert!(!already, "fi ligature should not already be NFKD");
}
// ---------------------------------------------------------------------------
// Already normalized: returns true, appends input verbatim
// ---------------------------------------------------------------------------

#[test]
fn nfc_normalize_to_already_normalized_returns_true() {
    let n = NfcNormalizer::new();
    let input = "hello";
    let mut buf = String::new();
    let already = n.normalize_to(input, &mut buf);
    assert!(already);
    assert_eq!(buf, input);
}

#[test]
fn nfd_normalize_to_already_normalized_returns_true() {
    let n = NfdNormalizer::new();
    let input = "hello";
    let mut buf = String::new();
    let already = n.normalize_to(input, &mut buf);
    assert!(already);
    assert_eq!(buf, input);
}

#[test]
fn nfkc_normalize_to_already_normalized_returns_true() {
    let n = NfkcNormalizer::new();
    let input = "hello";
    let mut buf = String::new();
    let already = n.normalize_to(input, &mut buf);
    assert!(already);
    assert_eq!(buf, input);
}

#[test]
fn nfkd_normalize_to_already_normalized_returns_true() {
    let n = NfkdNormalizer::new();
    let input = "hello";
    let mut buf = String::new();
    let already = n.normalize_to(input, &mut buf);
    assert!(already);
    assert_eq!(buf, input);
}
// ---------------------------------------------------------------------------
// Non-empty buffer: existing prefix preserved, only suffix written
// ---------------------------------------------------------------------------

#[test]
fn nfc_normalize_to_preserves_prefix() {
    let n = NfcNormalizer::new();
    let mut buf = String::from("PREFIX-");
    let already = n.normalize_to("A\u{030A}", &mut buf);
    assert!(!already);
    assert_eq!(buf, "PREFIX-\u{00C5}");
}

#[test]
fn nfd_normalize_to_preserves_prefix() {
    let n = NfdNormalizer::new();
    let mut buf = String::from("PREFIX-");
    let already = n.normalize_to("\u{00C5}", &mut buf);
    assert!(!already);
    assert_eq!(buf, "PREFIX-A\u{030A}");
}

#[test]
fn nfkc_normalize_to_preserves_prefix() {
    let n = NfkcNormalizer::new();
    let mut buf = String::from("PREFIX-");
    let already = n.normalize_to("\u{FB01}", &mut buf);
    assert!(!already);
    assert_eq!(buf, "PREFIX-fi");
}

#[test]
fn nfkd_normalize_to_preserves_prefix() {
    let n = NfkdNormalizer::new();
    let mut buf = String::from("PREFIX-");
    let already = n.normalize_to("\u{FB01}", &mut buf);
    assert!(!already);
    assert_eq!(buf, "PREFIX-fi");
}
// ---------------------------------------------------------------------------
// Repeated calls concatenate correctly
// ---------------------------------------------------------------------------

#[test]
fn nfc_normalize_to_repeated_calls_concatenate() {
    let n = NfcNormalizer::new();
    let mut buf = String::new();
    n.normalize_to("A\u{030A}", &mut buf);
    n.normalize_to("O\u{0308}", &mut buf); // O + diaeresis → Ö
    assert_eq!(buf, "\u{00C5}\u{00D6}");
}

#[test]
fn nfd_normalize_to_repeated_calls_concatenate() {
    let n = NfdNormalizer::new();
    let mut buf = String::new();
    n.normalize_to("\u{00C5}", &mut buf);
    n.normalize_to("\u{00D6}", &mut buf);
    assert_eq!(buf, "A\u{030A}O\u{0308}");
}

// ---------------------------------------------------------------------------
// Empty input: returns true, buf unchanged
// ---------------------------------------------------------------------------

#[test]
fn nfc_normalize_to_empty_input_returns_true() {
    let n = NfcNormalizer::new();
    let mut buf = String::from("keep-me");
    let already = n.normalize_to("", &mut buf);
    assert!(already);
    assert_eq!(buf, "keep-me");
}

#[test]
fn nfd_normalize_to_empty_input_returns_true() {
    let n = NfdNormalizer::new();
    let mut buf = String::from("keep-me");
    let already = n.normalize_to("", &mut buf);
    assert!(already);
    assert_eq!(buf, "keep-me");
}

#[test]
fn nfkc_normalize_to_empty_input_returns_true() {
    let n = NfkcNormalizer::new();
    let mut buf = String::from("keep-me");
    let already = n.normalize_to("", &mut buf);
    assert!(already);
    assert_eq!(buf, "keep-me");
}

#[test]
fn nfkd_normalize_to_empty_input_returns_true() {
    let n = NfkdNormalizer::new();
    let mut buf = String::from("keep-me");
    let already = n.normalize_to("", &mut buf);
    assert!(already);
    assert_eq!(buf, "keep-me");
}
// ---------------------------------------------------------------------------
// Larger inputs: ≥ 1 KiB ASCII and ≥ 1 KiB mixed multi-byte
// ---------------------------------------------------------------------------

#[test]
fn nfc_normalize_to_large_ascii_matches_normalize() {
    let n = NfcNormalizer::new();
    let input: String = "a".repeat(2048);
    let mut buf = String::new();
    let already = n.normalize_to(&input, &mut buf);
    assert!(already);
    assert_eq!(buf, input);
}

#[test]
fn nfd_normalize_to_large_mixed_matches_normalize() {
    let n = NfdNormalizer::new();
    // Repeat a 3-char group (Å + combining ring + ASCII) until > 1 KiB.
    let chunk = "\u{00C5}\u{030A}x";
    let mut input = String::new();
    while input.len() < 1200 {
        input.push_str(chunk);
    }
    let mut buf = String::new();
    n.normalize_to(&input, &mut buf);
    assert_eq!(buf, &*n.normalize(&input));
}

#[test]
fn nfkc_normalize_to_large_compat_matches_normalize() {
    let n = NfkcNormalizer::new();
    let chunk = "\u{FB01}\u{FB02}abc"; // ﬁ ﬂ abc
    let mut input = String::new();
    while input.len() < 1200 {
        input.push_str(chunk);
    }
    let mut buf = String::new();
    n.normalize_to(&input, &mut buf);
    assert_eq!(buf, &*n.normalize(&input));
}
