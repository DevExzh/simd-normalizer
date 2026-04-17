//! Value-assertion edge-case tests for `normalize_for_matching_utf16`.
//!
//! Closes the gap identified in
//! `docs/superpowers/specs/2026-04-17-full-edge-case-coverage-design.md`
//! section 2. The existing `tests/matching_edge_cases.rs` covers empty,
//! BMP-only, round-trip, and mixed cases via shape assertions; this file
//! adds (a) exact-value assertions for NFKC-expanding inputs and
//! combining-mark composition, and (b) a 500-case property equating
//! `normalize_for_matching_utf16(s)` with
//! `normalize_for_matching(s).encode_utf16().collect::<Vec<u16>>()`.

use proptest::prelude::*;
use simd_normalizer::matching::{
    MatchingOptions, normalize_for_matching, normalize_for_matching_utf16,
};

fn default_opts() -> MatchingOptions {
    MatchingOptions::default()
}

// ---------------------------------------------------------------------------
// NFKC-expansion value assertions (new vs tests/matching_edge_cases.rs,
// which only does round-trip shape checks)
// ---------------------------------------------------------------------------

#[test]
fn utf16_supplementary_emits_exactly_one_surrogate_pair() {
    // U+1F600 GRINNING FACE — single scalar, exactly two UTF-16 code units.
    let out = normalize_for_matching_utf16("\u{1F600}", &default_opts());
    assert_eq!(out.len(), 2, "supplementary char must be exactly a surrogate pair");
    assert!(
        (0xD800..=0xDBFF).contains(&out[0]),
        "first unit must be a high surrogate, got {:04X}",
        out[0]
    );
    assert!(
        (0xDC00..=0xDFFF).contains(&out[1]),
        "second unit must be a low surrogate, got {:04X}",
        out[1]
    );
    let decoded = String::from_utf16(&out).expect("valid surrogate pair");
    assert_eq!(decoded, normalize_for_matching("\u{1F600}", &default_opts()));
}

#[test]
fn utf16_nfkc_expanding_fullwidth() {
    // U+FF21 FULLWIDTH LATIN CAPITAL LETTER A → NFKC "A" → casefold "a".
    let out = normalize_for_matching_utf16("\u{FF21}", &default_opts());
    let decoded = String::from_utf16(&out).expect("valid UTF-16");
    assert_eq!(decoded, "a");
    assert_eq!(out.len(), 1);
}

#[test]
fn utf16_nfkc_expanding_ligature() {
    // U+FB01 ﬁ → NFKC "fi" (1 input char → 2 output code units).
    let out = normalize_for_matching_utf16("\u{FB01}", &default_opts());
    let decoded = String::from_utf16(&out).expect("valid UTF-16");
    assert_eq!(decoded, "fi");
    assert_eq!(out.len(), 2);
}

#[test]
fn utf16_nfkc_expanding_superscript() {
    // U+00B2 SUPERSCRIPT TWO → NFKC "2".
    let out = normalize_for_matching_utf16("\u{00B2}", &default_opts());
    let decoded = String::from_utf16(&out).expect("valid UTF-16");
    assert_eq!(decoded, "2");
    assert_eq!(out.len(), 1);
}

#[test]
fn utf16_combining_marks_casefold_decomposed() {
    // A + combining acute (U+0301) round-trips through the matching pipeline
    // as lowercase + combining acute. The pipeline's final stage decomposes
    // (UTS #39 skeleton → NFD), so precomposed Á (U+00C1) is NOT produced.
    // Input and both pre-normalized equivalents must map to the same output.
    let expected: Vec<u16> = "a\u{0301}".encode_utf16().collect();
    for input in ["A\u{0301}", "\u{00C1}", "\u{00E1}"] {
        let out = normalize_for_matching_utf16(input, &default_opts());
        assert_eq!(out, expected, "unexpected output for input {input:?}");
        assert_eq!(out.len(), 2);
    }
}

// ---------------------------------------------------------------------------
// Equivalence property vs normalize_for_matching().encode_utf16()
// ---------------------------------------------------------------------------

fn matching_string_strategy() -> impl Strategy<Value = String> {
    let ranges = prop::char::ranges(std::borrow::Cow::Borrowed(&[
        // ASCII printable
        '\u{0020}'..='\u{007E}',
        // Latin-1 Supplement (exercises NFC composition)
        '\u{00C0}'..='\u{00FF}',
        // Combining Diacritical Marks
        '\u{0300}'..='\u{036F}',
        // Cyrillic (confusable with Latin)
        '\u{0400}'..='\u{04FF}',
        // Alphabetic Presentation Forms (ligatures: exercise NFKC expansion)
        '\u{FB00}'..='\u{FB06}',
        // Halfwidth and Fullwidth Forms (NFKC expansion)
        '\u{FF01}'..='\u{FF5E}',
        // Emoticons (supplementary — surrogate pairs)
        '\u{1F600}'..='\u{1F64F}',
    ]));
    prop::collection::vec(ranges, 0..32).prop_map(|chars| chars.into_iter().collect())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn utf16_equals_normalize_for_matching_then_encode(s in matching_string_strategy()) {
        let opts = MatchingOptions::default();
        let direct = normalize_for_matching_utf16(&s, &opts);
        let indirect: Vec<u16> = normalize_for_matching(&s, &opts).encode_utf16().collect();
        prop_assert_eq!(direct, indirect);
    }
}
