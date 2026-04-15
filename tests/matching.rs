//! Integration tests for the fused matching normalization pipeline.

use simd_normalizer::matching::{MatchingOptions, matches_normalized, normalize_for_matching, normalize_for_matching_utf16};
use simd_normalizer::CaseFoldMode;

fn default_opts() -> MatchingOptions {
    MatchingOptions::default()
}

fn turkish_opts() -> MatchingOptions {
    MatchingOptions {
        case_fold: CaseFoldMode::Turkish,
    }
}

// ---------------------------------------------------------------------------
// Core requirement: "file" / "fıle" / "File" / "FıLE" equivalence
// ---------------------------------------------------------------------------

#[test]
fn file_variants_produce_identical_output() {
    let opts = default_opts();
    let canonical = normalize_for_matching("file", &opts);

    assert_eq!(normalize_for_matching("File", &opts), canonical, "'File' should match 'file'");
    assert_eq!(normalize_for_matching("FILE", &opts), canonical, "'FILE' should match 'file'");

    // fıle — Turkish dotless-ı (U+0131)
    let result_fıle = normalize_for_matching("f\u{0131}le", &opts);
    assert_eq!(result_fıle, canonical, "'fıle' should match 'file'");

    // FıLE — mixed case + Turkish dotless-ı
    let result_fıle_mixed = normalize_for_matching("F\u{0131}LE", &opts);
    assert_eq!(result_fıle_mixed, canonical, "'FıLE' should match 'file'");
}

// ---------------------------------------------------------------------------
// Case folding through the pipeline
// ---------------------------------------------------------------------------

#[test]
fn case_insensitive_matching() {
    let opts = default_opts();
    assert!(matches_normalized("Hello", "hello", &opts));
    assert!(matches_normalized("WORLD", "world", &opts));
    assert!(matches_normalized("CaFé", "café", &opts));
}

#[test]
fn case_insensitive_greek() {
    let opts = default_opts();
    // ΑΒΓΔ → αβγδ
    assert!(matches_normalized(
        "\u{0391}\u{0392}\u{0393}\u{0394}",
        "\u{03B1}\u{03B2}\u{03B3}\u{03B4}",
        &opts,
    ));
}

// ---------------------------------------------------------------------------
// Confusable detection through the pipeline
// ---------------------------------------------------------------------------

#[test]
fn confusable_latin_cyrillic() {
    let opts = default_opts();
    assert!(matches_normalized("a", "\u{0430}", &opts));
    assert!(matches_normalized("e", "\u{0435}", &opts));
    assert!(matches_normalized("o", "\u{043E}", &opts));
}

#[test]
fn confusable_word_level() {
    let opts = default_opts();
    // "apple" vs mixed Cyrillic
    let latin = "apple";
    let mixed = "\u{0430}\u{0440}\u{0440}l\u{0435}";
    assert!(matches_normalized(latin, mixed, &opts));
}

// ---------------------------------------------------------------------------
// NFKC compatibility through the pipeline
// ---------------------------------------------------------------------------

#[test]
fn nfkc_fullwidth_matching() {
    let opts = default_opts();
    // Fullwidth 'A' (U+FF21) → 'A' → 'a' (after NFKC + casefold)
    assert!(matches_normalized("\u{FF21}", "a", &opts));
}

#[test]
fn nfkc_superscript_matching() {
    let opts = default_opts();
    // Superscript '2' (U+00B2) → '2'
    assert!(matches_normalized("\u{00B2}", "2", &opts));
}

#[test]
fn nfkc_ligature_matching() {
    let opts = default_opts();
    // fi ligature (U+FB01) NFKC-decomposes to "fi"
    assert!(matches_normalized("\u{FB01}", "fi", &opts));
}

// ---------------------------------------------------------------------------
// Turkish mode
// ---------------------------------------------------------------------------

#[test]
fn turkish_dotless_i_matching() {
    let opts = turkish_opts();
    // In Turkish: I → ı, so "Istanbul" matches "ıstanbul"
    assert!(matches_normalized("Istanbul", "\u{0131}stanbul", &opts));
}

#[test]
fn turkish_dotted_capital_i_matching() {
    let opts = turkish_opts();
    // In Turkish: İ (U+0130) → i, so "İstanbul" matches "istanbul"
    assert!(matches_normalized("\u{0130}stanbul", "istanbul", &opts));
}

// ---------------------------------------------------------------------------
// Non-matching pairs
// ---------------------------------------------------------------------------

#[test]
fn different_words_dont_match() {
    let opts = default_opts();
    assert!(!matches_normalized("hello", "world", &opts));
    assert!(!matches_normalized("cat", "dog", &opts));
    assert!(!matches_normalized("file", "pile", &opts));
}

// ---------------------------------------------------------------------------
// Idempotence
// ---------------------------------------------------------------------------

#[test]
fn matching_is_idempotent() {
    let opts = default_opts();
    let inputs = [
        "hello",
        "File",
        "CAFÉ",
        "\u{0430}\u{0440}\u{0440}l\u{0435}",
        "\u{00C0}\u{00C9}\u{00D6}",
        "\u{1F600}",
        "\u{FF21}\u{FF22}\u{FF23}",
    ];
    for input in &inputs {
        let once = normalize_for_matching(input, &opts);
        let twice = normalize_for_matching(&once, &opts);
        assert_eq!(once, twice, "not idempotent for {:?}", input);
    }
}

// ---------------------------------------------------------------------------
// UTF-16 encoding
// ---------------------------------------------------------------------------

#[test]
fn utf16_roundtrip() {
    let opts = default_opts();
    let inputs = ["hello", "File", "CAFÉ", "\u{1F600}"];
    for input in &inputs {
        let utf16 = normalize_for_matching_utf16(input, &opts);
        let decoded = String::from_utf16(&utf16).expect("valid UTF-16");
        assert_eq!(decoded, normalize_for_matching(input, &opts));
    }
}

#[test]
fn utf16_supplementary_surrogates() {
    let opts = default_opts();
    // Emoji: U+1F600 — encodes as surrogate pair in UTF-16
    let utf16 = normalize_for_matching_utf16("\u{1F600}", &opts);
    assert!(utf16.len() >= 2, "supplementary char should produce surrogate pair");
    let decoded = String::from_utf16(&utf16).expect("valid UTF-16");
    assert_eq!(decoded, normalize_for_matching("\u{1F600}", &opts));
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[test]
fn empty_string() {
    assert_eq!(normalize_for_matching("", &default_opts()), "");
    assert!(matches_normalized("", "", &default_opts()));
}

#[test]
fn single_char() {
    let opts = default_opts();
    let _ = normalize_for_matching("a", &opts);
    let _ = normalize_for_matching("A", &opts);
    let _ = normalize_for_matching("\u{0430}", &opts);
}

#[test]
fn long_input_no_panic() {
    let opts = default_opts();
    let input = "The quick brown fox ".repeat(1000);
    let result = normalize_for_matching(&input, &opts);
    assert!(!result.is_empty());
}

#[test]
fn combining_marks_only() {
    let opts = default_opts();
    // Standalone combining marks shouldn't panic.
    let _ = normalize_for_matching("\u{0300}\u{0301}\u{0302}", &opts);
}
