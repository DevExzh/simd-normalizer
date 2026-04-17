//! Supplementary plane edge case tests.
//!
//! Tests Unicode characters outside the BMP (U+10000 and above) that exercise
//! 4-byte UTF-8 encoding, supplementary decompositions, compatibility mappings,
//! case folding, and SIMD chunk boundary handling for multi-byte sequences.
//!
//! Categories:
//! 1. Musical symbols with canonical decompositions (composition exclusions)
//! 2. CJK Compatibility Ideographs Supplement (U+2F800 range)
//! 3. Mathematical Alphanumeric Symbols (U+1D400 range) -- NFKC/NFKD
//! 4. Supplementary character case folding (math bold, Deseret)
//! 5. 4-byte UTF-8 supplementary chars at every SIMD chunk offset position
//! 6. Cross-validation of all results against ICU4X

use icu_normalizer::{ComposingNormalizerBorrowed, DecomposingNormalizerBorrowed};
use simd_normalizer::matching::{MatchingOptions, normalize_for_matching};
use simd_normalizer::{CaseFoldMode, UnicodeNormalization, casefold};

// ---------------------------------------------------------------------------
// ICU4X reference helpers
// ---------------------------------------------------------------------------

fn icu_nfc(s: &str) -> String {
    ComposingNormalizerBorrowed::new_nfc()
        .normalize(s)
        .into_owned()
}

fn icu_nfd(s: &str) -> String {
    DecomposingNormalizerBorrowed::new_nfd()
        .normalize(s)
        .into_owned()
}

fn icu_nfkc(s: &str) -> String {
    ComposingNormalizerBorrowed::new_nfkc()
        .normalize(s)
        .into_owned()
}

fn icu_nfkd(s: &str) -> String {
    DecomposingNormalizerBorrowed::new_nfkd()
        .normalize(s)
        .into_owned()
}

// ---------------------------------------------------------------------------
// Cross-validation assertion helpers
// ---------------------------------------------------------------------------

/// Assert NFD output matches expected AND matches ICU4X.
fn assert_nfd(input: &str, expected: &str) {
    let simd_result = input.nfd();
    let icu_result = icu_nfd(input);
    assert_eq!(
        &*simd_result, expected,
        "NFD mismatch for {:?}: simd={:?}, expected={:?}",
        input, simd_result, expected
    );
    assert_eq!(
        &*simd_result, &*icu_result,
        "NFD cross-validation failed for {:?}: simd={:?}, icu={:?}",
        input, simd_result, icu_result
    );
}

/// Assert NFC output matches expected AND matches ICU4X.
fn assert_nfc(input: &str, expected: &str) {
    let simd_result = input.nfc();
    let icu_result = icu_nfc(input);
    assert_eq!(
        &*simd_result, expected,
        "NFC mismatch for {:?}: simd={:?}, expected={:?}",
        input, simd_result, expected
    );
    assert_eq!(
        &*simd_result, &*icu_result,
        "NFC cross-validation failed for {:?}: simd={:?}, icu={:?}",
        input, simd_result, icu_result
    );
}

/// Assert NFKD output matches expected AND matches ICU4X.
fn assert_nfkd(input: &str, expected: &str) {
    let simd_result = input.nfkd();
    let icu_result = icu_nfkd(input);
    assert_eq!(
        &*simd_result, expected,
        "NFKD mismatch for {:?}: simd={:?}, expected={:?}",
        input, simd_result, expected
    );
    assert_eq!(
        &*simd_result, &*icu_result,
        "NFKD cross-validation failed for {:?}: simd={:?}, icu={:?}",
        input, simd_result, icu_result
    );
}

/// Assert NFKC output matches expected AND matches ICU4X.
fn assert_nfkc(input: &str, expected: &str) {
    let simd_result = input.nfkc();
    let icu_result = icu_nfkc(input);
    assert_eq!(
        &*simd_result, expected,
        "NFKC mismatch for {:?}: simd={:?}, expected={:?}",
        input, simd_result, expected
    );
    assert_eq!(
        &*simd_result, &*icu_result,
        "NFKC cross-validation failed for {:?}: simd={:?}, icu={:?}",
        input, simd_result, icu_result
    );
}

/// Assert all four normalization forms match ICU4X for the given input.
fn assert_all_forms_match_icu(input: &str) {
    let nfc_simd = input.nfc();
    let nfd_simd = input.nfd();
    let nfkc_simd = input.nfkc();
    let nfkd_simd = input.nfkd();

    assert_eq!(
        &*nfc_simd,
        &*icu_nfc(input),
        "NFC cross-validation failed for input ({} bytes)",
        input.len()
    );
    assert_eq!(
        &*nfd_simd,
        &*icu_nfd(input),
        "NFD cross-validation failed for input ({} bytes)",
        input.len()
    );
    assert_eq!(
        &*nfkc_simd,
        &*icu_nfkc(input),
        "NFKC cross-validation failed for input ({} bytes)",
        input.len()
    );
    assert_eq!(
        &*nfkd_simd,
        &*icu_nfkd(input),
        "NFKD cross-validation failed for input ({} bytes)",
        input.len()
    );
}

/// Assert is_normalized checks match ICU4X for the given input.
fn assert_is_normalized_matches_icu(input: &str) {
    let icu_nfc = ComposingNormalizerBorrowed::new_nfc();
    let icu_nfd = DecomposingNormalizerBorrowed::new_nfd();
    let icu_nfkc = ComposingNormalizerBorrowed::new_nfkc();
    let icu_nfkd = DecomposingNormalizerBorrowed::new_nfkd();

    assert_eq!(
        input.is_nfc(),
        icu_nfc.is_normalized(input),
        "is_nfc mismatch for {:?}",
        input
    );
    assert_eq!(
        input.is_nfd(),
        icu_nfd.is_normalized(input),
        "is_nfd mismatch for {:?}",
        input
    );
    assert_eq!(
        input.is_nfkc(),
        icu_nfkc.is_normalized(input),
        "is_nfkc mismatch for {:?}",
        input
    );
    assert_eq!(
        input.is_nfkd(),
        icu_nfkd.is_normalized(input),
        "is_nfkd mismatch for {:?}",
        input
    );
}

/// Build a string of exactly `n` ASCII bytes using a repeating pattern.
fn ascii_pad(n: usize) -> String {
    let pattern = b"abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ_-";
    let mut s = String::with_capacity(n);
    for i in 0..n {
        s.push(pattern[i % pattern.len()] as char);
    }
    debug_assert_eq!(s.len(), n);
    s
}

// ===========================================================================
// 1. Musical symbols with canonical decompositions (composition exclusions)
// ===========================================================================

#[test]
fn musical_half_note_decomposition() {
    // U+1D15E (MUSICAL SYMBOL HALF NOTE) canonically decomposes to
    // U+1D157 (VOID NOTEHEAD) + U+1D165 (COMBINING STEM).
    // This is a composition exclusion, so NFC does NOT recompose it.
    let half_note = "\u{1D15E}";
    let expected_decomposed = "\u{1D157}\u{1D165}";

    assert_nfd(half_note, expected_decomposed);
    assert_nfkd(half_note, expected_decomposed);
    // Composition exclusion: NFC and NFKC produce the same decomposed form.
    assert_nfc(half_note, expected_decomposed);
    assert_nfkc(half_note, expected_decomposed);
}

#[test]
fn musical_quarter_note_decomposition() {
    // U+1D15F (MUSICAL SYMBOL QUARTER NOTE) canonically decomposes to
    // U+1D158 (NOTEPAD BLACK) + U+1D165 (COMBINING STEM).
    let quarter_note = "\u{1D15F}";
    let expected_decomposed = "\u{1D158}\u{1D165}";

    assert_nfd(quarter_note, expected_decomposed);
    assert_nfkd(quarter_note, expected_decomposed);
    assert_nfc(quarter_note, expected_decomposed);
    assert_nfkc(quarter_note, expected_decomposed);
}

#[test]
fn musical_eighth_note_multi_step_decomposition() {
    // U+1D160 (MUSICAL SYMBOL EIGHTH NOTE) decomposes to
    // U+1D15F (QUARTER NOTE) + U+1D16E (COMBINING FLAG-1),
    // and U+1D15F itself decomposes to U+1D158 + U+1D165.
    // Full decomposition: U+1D158 + U+1D165 + U+1D16E.
    let eighth_note = "\u{1D160}";
    let expected_decomposed = "\u{1D158}\u{1D165}\u{1D16E}";

    assert_nfd(eighth_note, expected_decomposed);
    assert_nfkd(eighth_note, expected_decomposed);
    // Composition exclusions: NFC/NFKC produce the same decomposed form.
    assert_nfc(eighth_note, expected_decomposed);
    assert_nfkc(eighth_note, expected_decomposed);
}

#[test]
fn musical_symbols_is_normalized_checks() {
    // The composed form (U+1D15E) is NOT normalized in any form because it
    // has a canonical decomposition.
    let half_note = "\u{1D15E}";
    assert!(!half_note.is_nfd(), "U+1D15E should not be NFD");
    assert!(!half_note.is_nfc(), "U+1D15E should not be NFC");
    assert!(!half_note.is_nfkd(), "U+1D15E should not be NFKD");
    assert!(!half_note.is_nfkc(), "U+1D15E should not be NFKC");
    assert_is_normalized_matches_icu(half_note);

    // The decomposed form IS normalized in all forms (composition exclusion).
    let decomposed = "\u{1D157}\u{1D165}";
    assert!(decomposed.is_nfd(), "decomposed half note should be NFD");
    assert!(decomposed.is_nfc(), "decomposed half note should be NFC (composition exclusion)");
    assert!(decomposed.is_nfkd(), "decomposed half note should be NFKD");
    assert!(decomposed.is_nfkc(), "decomposed half note should be NFKC");
    assert_is_normalized_matches_icu(decomposed);
}

#[test]
fn musical_symbols_nfc_nfkc_agreement() {
    // For musical symbols with canonical (not compatibility) decompositions,
    // NFC and NFKC should agree, and NFD and NFKD should agree.
    let inputs = ["\u{1D15E}", "\u{1D15F}", "\u{1D160}"];
    for input in &inputs {
        let nfc_result = input.nfc();
        let nfkc_result = input.nfkc();
        assert_eq!(
            &*nfc_result, &*nfkc_result,
            "NFC and NFKC should agree for musical symbol {:?}",
            input
        );

        let nfd_result = input.nfd();
        let nfkd_result = input.nfkd();
        assert_eq!(
            &*nfd_result, &*nfkd_result,
            "NFD and NFKD should agree for musical symbol {:?}",
            input
        );
    }
}

#[test]
fn musical_symbol_with_surrounding_text() {
    // Musical symbol embedded in ASCII text.
    let input = "Music: \u{1D15E} is a half note";
    assert_all_forms_match_icu(input);

    // Multiple musical symbols in sequence.
    let input2 = "\u{1D15E}\u{1D15F}\u{1D160}";
    assert_all_forms_match_icu(input2);
}

// ===========================================================================
// 2. CJK Compatibility Ideographs Supplement (U+2F800-U+2FA1F)
// ===========================================================================

#[test]
fn cjk_compat_supplement_canonical_decomposition() {
    // These have canonical decompositions to CJK Unified Ideographs.
    // All four normalization forms should produce the unified ideograph.
    let cases: &[(&str, &str)] = &[
        ("\u{2F800}", "\u{4E3D}"), // CJK COMPATIBILITY IDEOGRAPH-2F800 -> U+4E3D
        ("\u{2F801}", "\u{4E38}"), // CJK COMPATIBILITY IDEOGRAPH-2F801 -> U+4E38
        ("\u{2F802}", "\u{4E41}"), // CJK COMPATIBILITY IDEOGRAPH-2F802 -> U+4E41
        ("\u{2F804}", "\u{4F60}"), // CJK COMPATIBILITY IDEOGRAPH-2F804 -> U+4F60
        ("\u{2F80A}", "\u{50E7}"), // CJK COMPATIBILITY IDEOGRAPH-2F80A -> U+50E7
    ];

    for &(input, expected) in cases {
        assert_nfd(input, expected);
        assert_nfc(input, expected);
        assert_nfkd(input, expected);
        assert_nfkc(input, expected);
    }
}

#[test]
fn cjk_compat_supplement_is_not_normalized() {
    // The compatibility ideograph forms are not normalized in any form.
    let compat_chars = ["\u{2F800}", "\u{2F801}", "\u{2F802}", "\u{2F804}", "\u{2F80A}"];
    for input in &compat_chars {
        assert!(!input.is_nfd(), "{:?} should not be NFD", input);
        assert!(!input.is_nfc(), "{:?} should not be NFC", input);
        assert!(!input.is_nfkd(), "{:?} should not be NFKD", input);
        assert!(!input.is_nfkc(), "{:?} should not be NFKC", input);
        assert_is_normalized_matches_icu(input);
    }
}

#[test]
fn cjk_compat_supplement_unified_form_is_normalized() {
    // The unified ideograph targets should be normalized in all forms.
    let unified_chars = ["\u{4E3D}", "\u{4E38}", "\u{4E41}", "\u{4F60}", "\u{50E7}"];
    for input in &unified_chars {
        assert!(input.is_nfd(), "{:?} should be NFD", input);
        assert!(input.is_nfc(), "{:?} should be NFC", input);
        assert!(input.is_nfkd(), "{:?} should be NFKD", input);
        assert!(input.is_nfkc(), "{:?} should be NFKC", input);
        assert_is_normalized_matches_icu(input);
    }
}

#[test]
fn cjk_compat_supplement_in_text() {
    // CJK compat ideographs embedded in mixed text.
    let input = "CJK: \u{2F800}\u{2F801} test";
    let expected_nfd = "CJK: \u{4E3D}\u{4E38} test";
    assert_nfd(input, expected_nfd);
    assert_nfc(input, expected_nfd);
    assert_all_forms_match_icu(input);
}

// ===========================================================================
// 3. Mathematical Alphanumeric Symbols (U+1D400-U+1D7FF)
// ===========================================================================

#[test]
fn math_bold_nfkc_decomposition() {
    // Mathematical Bold letters have compatibility decompositions to basic Latin.
    // NFC/NFD leave them unchanged; NFKC/NFKD decompose them.
    let cases: &[(&str, &str, &str)] = &[
        ("\u{1D400}", "A", "MATH BOLD CAPITAL A"),
        ("\u{1D41A}", "a", "MATH BOLD SMALL A"),
        ("\u{1D401}", "B", "MATH BOLD CAPITAL B"),
        ("\u{1D41B}", "b", "MATH BOLD SMALL B"),
    ];

    for &(input, expected_compat, label) in cases {
        // NFC/NFD: unchanged (no canonical decomposition).
        assert_nfc(input, input);
        assert_nfd(input, input);
        // NFKC/NFKD: compatibility decomposition to basic Latin.
        assert_nfkc(input, expected_compat);
        assert_nfkd(input, expected_compat);
        // Verify is_normalized checks.
        assert!(input.is_nfc(), "{} should be NFC", label);
        assert!(input.is_nfd(), "{} should be NFD", label);
        assert!(!input.is_nfkc(), "{} should NOT be NFKC", label);
        assert!(!input.is_nfkd(), "{} should NOT be NFKD", label);
        assert_is_normalized_matches_icu(input);
    }
}

#[test]
fn math_italic_nfkc_decomposition() {
    let cases: &[(&str, &str)] = &[
        ("\u{1D434}", "A"), // MATHEMATICAL ITALIC CAPITAL A
        ("\u{1D44E}", "a"), // MATHEMATICAL ITALIC SMALL A
    ];
    for &(input, expected) in cases {
        assert_nfc(input, input);
        assert_nfd(input, input);
        assert_nfkc(input, expected);
        assert_nfkd(input, expected);
    }
}

#[test]
fn math_bold_italic_nfkc_decomposition() {
    let cases: &[(&str, &str)] = &[
        ("\u{1D468}", "A"), // MATHEMATICAL BOLD ITALIC CAPITAL A
        ("\u{1D482}", "a"), // MATHEMATICAL BOLD ITALIC SMALL A
    ];
    for &(input, expected) in cases {
        assert_nfc(input, input);
        assert_nfd(input, input);
        assert_nfkc(input, expected);
        assert_nfkd(input, expected);
    }
}

#[test]
fn math_script_nfkc_decomposition() {
    // MATHEMATICAL SCRIPT CAPITAL A -> A
    assert_nfkc("\u{1D49C}", "A");
    assert_nfkd("\u{1D49C}", "A");
    assert_nfc("\u{1D49C}", "\u{1D49C}");
    assert_nfd("\u{1D49C}", "\u{1D49C}");
}

#[test]
fn math_fraktur_nfkc_decomposition() {
    // MATHEMATICAL FRAKTUR CAPITAL A -> A
    assert_nfkc("\u{1D504}", "A");
    assert_nfkd("\u{1D504}", "A");
    assert_nfc("\u{1D504}", "\u{1D504}");
    assert_nfd("\u{1D504}", "\u{1D504}");
}

#[test]
fn math_doublestruck_nfkc_decomposition() {
    // MATHEMATICAL DOUBLE-STRUCK CAPITAL A -> A
    assert_nfkc("\u{1D538}", "A");
    assert_nfkd("\u{1D538}", "A");
    assert_nfc("\u{1D538}", "\u{1D538}");
    assert_nfd("\u{1D538}", "\u{1D538}");
}

#[test]
fn math_bold_digits_nfkc_decomposition() {
    // MATHEMATICAL BOLD DIGIT ZERO -> 0
    assert_nfkc("\u{1D7CE}", "0");
    assert_nfkd("\u{1D7CE}", "0");
    assert_nfc("\u{1D7CE}", "\u{1D7CE}");
    assert_nfd("\u{1D7CE}", "\u{1D7CE}");

    // MATHEMATICAL BOLD DIGIT NINE -> 9
    assert_nfkc("\u{1D7D7}", "9");
    assert_nfkd("\u{1D7D7}", "9");
}

#[test]
fn math_symbols_string_nfkc() {
    // A string of mixed math styles should all decompose under NFKC/NFKD.
    // Bold A + Italic a + Script A + Fraktur A
    let input = "\u{1D400}\u{1D44E}\u{1D49C}\u{1D504}";
    assert_nfkc(input, "AaAA");
    assert_nfkd(input, "AaAA");
    // NFC/NFD: unchanged.
    assert_nfc(input, input);
    assert_nfd(input, input);
    assert_all_forms_match_icu(input);
}

// ===========================================================================
// 4. Supplementary character case folding
// ===========================================================================

#[test]
fn math_bold_capital_a_casefold_via_nfkc() {
    // U+1D400 (MATH BOLD CAPITAL A) has no direct case fold mapping,
    // but NFKC maps it to "A", which then casefolds to "a".
    let nfkc_result = "\u{1D400}".nfkc();
    assert_eq!(&*nfkc_result, "A");
    let folded = casefold(&nfkc_result, CaseFoldMode::Standard);
    assert_eq!(&*folded, "a");
}

#[test]
fn math_bold_small_a_casefold_via_nfkc() {
    // U+1D41A (MATH BOLD SMALL A) -> NFKC -> "a" -> casefold -> "a" (unchanged).
    let nfkc_result = "\u{1D41A}".nfkc();
    assert_eq!(&*nfkc_result, "a");
    let folded = casefold(&nfkc_result, CaseFoldMode::Standard);
    assert_eq!(&*folded, "a");
}

#[test]
fn deseret_capital_casefold() {
    // Deseret letters (U+10400-U+1044F): capitals fold to lowercase.
    // U+10400 (DESERET CAPITAL LONG I) -> U+10428 (DESERET SMALL LONG I)
    let capital = "\u{10400}";
    let expected_lower = "\u{10428}";
    let folded = casefold(capital, CaseFoldMode::Standard);
    assert_eq!(
        &*folded, expected_lower,
        "Deseret capital U+10400 should casefold to U+10428"
    );
}

#[test]
fn deseret_lowercase_casefold_unchanged() {
    // U+10428 (DESERET SMALL LONG I) should be unchanged by casefold.
    let lower = "\u{10428}";
    let folded = casefold(lower, CaseFoldMode::Standard);
    assert_eq!(
        &*folded, lower,
        "Deseret lowercase U+10428 should be unchanged by casefold"
    );
}

#[test]
fn deseret_range_casefold() {
    // Test several Deseret capital-to-lowercase pairs.
    let pairs: &[(char, char)] = &[
        ('\u{10400}', '\u{10428}'), // LONG I
        ('\u{10401}', '\u{10429}'), // LONG E
        ('\u{10402}', '\u{1042A}'), // LONG A
        ('\u{10410}', '\u{10438}'), // BEE
        ('\u{1041F}', '\u{10447}'), // EW
    ];
    for &(upper, lower) in pairs {
        let upper_str: String = core::iter::once(upper).collect();
        let folded = casefold(&upper_str, CaseFoldMode::Standard);
        let expected: String = core::iter::once(lower).collect();
        assert_eq!(
            &*folded, &expected,
            "Deseret U+{:04X} should casefold to U+{:04X}",
            upper as u32, lower as u32
        );
    }
}

#[test]
fn deseret_normalization_unchanged() {
    // Deseret letters have no decomposition mappings; all forms leave them unchanged.
    let capital = "\u{10400}";
    let lower = "\u{10428}";
    for input in &[capital, lower] {
        assert_nfc(input, input);
        assert_nfd(input, input);
        assert_nfkc(input, input);
        assert_nfkd(input, input);
        assert_is_normalized_matches_icu(input);
    }
}

#[test]
fn matching_pipeline_math_bold() {
    // The matching pipeline (NFKC + casefold + skeleton) should equate
    // Math Bold A and plain "a".
    let opts = MatchingOptions::default();
    let bold_a = normalize_for_matching("\u{1D400}", &opts);
    let plain_a = normalize_for_matching("a", &opts);
    assert_eq!(
        bold_a, plain_a,
        "Math Bold A should match 'a' through the matching pipeline"
    );
}

#[test]
fn matching_pipeline_math_styles_equivalent() {
    // All mathematical styles of "A" should match plain "a" through the pipeline.
    let opts = MatchingOptions::default();
    let reference = normalize_for_matching("a", &opts);
    let math_capitals = [
        "\u{1D400}", // Bold A
        "\u{1D434}", // Italic A
        "\u{1D468}", // Bold Italic A
        "\u{1D49C}", // Script A
        "\u{1D504}", // Fraktur A
        "\u{1D538}", // Double-struck A
    ];
    for input in &math_capitals {
        let result = normalize_for_matching(input, &opts);
        assert_eq!(
            result, reference,
            "Math symbol {:?} should match 'a' through matching pipeline",
            input
        );
    }
}

#[test]
fn matching_pipeline_deseret() {
    // Deseret capital and lowercase should match through the matching pipeline.
    let opts = MatchingOptions::default();
    let upper = normalize_for_matching("\u{10400}", &opts);
    let lower = normalize_for_matching("\u{10428}", &opts);
    assert_eq!(
        upper, lower,
        "Deseret U+10400 and U+10428 should match through matching pipeline"
    );
}

// ===========================================================================
// 5. 4-byte UTF-8 supplementary chars at every SIMD chunk offset position
// ===========================================================================

#[test]
fn musical_symbol_at_every_offset_in_16byte_window() {
    // Place a musical symbol (U+1D15E, which decomposes) at offsets 0..=15
    // within a 16-byte window. The 4-byte UTF-8 encoding tests the SIMD
    // scanner's ability to detect non-ASCII bytes and fall back to scalar
    // processing at various alignments.
    for offset in 0..=15 {
        let prefix = ascii_pad(offset);
        let suffix_len = 15usize.saturating_sub(offset);
        let suffix = ascii_pad(suffix_len);
        let input = format!("{}\u{1D15E}{}", prefix, suffix);

        assert_all_forms_match_icu(&input);
    }
}

#[test]
fn cjk_compat_at_every_offset_in_16byte_window() {
    // U+2F800 (CJK compat, decomposes to U+4E3D) at various offsets.
    for offset in 0..=15 {
        let prefix = ascii_pad(offset);
        let suffix_len = 15usize.saturating_sub(offset);
        let suffix = ascii_pad(suffix_len);
        let input = format!("{}\u{2F800}{}", prefix, suffix);

        assert_all_forms_match_icu(&input);
    }
}

#[test]
fn math_bold_at_every_offset_in_16byte_window() {
    // U+1D400 (MATH BOLD A, compatibility decomposition) at various offsets.
    for offset in 0..=15 {
        let prefix = ascii_pad(offset);
        let suffix_len = 15usize.saturating_sub(offset);
        let suffix = ascii_pad(suffix_len);
        let input = format!("{}\u{1D400}{}", prefix, suffix);

        assert_all_forms_match_icu(&input);
    }
}

#[test]
fn supplementary_char_at_64byte_chunk_boundary_offsets() {
    // Test a decomposing supplementary char at various positions relative to
    // the 64-byte SIMD chunk boundary.
    for ascii_prefix_len in 56..=68 {
        let prefix = ascii_pad(ascii_prefix_len);
        let input = format!("{}\u{1D15E}", prefix);
        assert_all_forms_match_icu(&input);
    }
}

#[test]
fn supplementary_char_at_128byte_chunk_boundary_offsets() {
    // Same test at the 128-byte boundary (two-chunk boundary).
    for ascii_prefix_len in 120..=132 {
        let prefix = ascii_pad(ascii_prefix_len);
        let input = format!("{}\u{1D15E}", prefix);
        assert_all_forms_match_icu(&input);
    }
}

#[test]
fn multiple_supplementary_chars_across_chunk() {
    // Multiple 4-byte chars in sequence near a chunk boundary.
    for start_offset in 56..=64 {
        let prefix = ascii_pad(start_offset);
        // Three musical symbols in sequence (12 bytes of 4-byte chars).
        let input = format!("{}\u{1D15E}\u{1D15F}\u{1D160}", prefix);
        assert_all_forms_match_icu(&input);
    }
}

#[test]
fn supplementary_char_interleaved_with_ascii_near_boundary() {
    // Supplementary chars interleaved with ASCII near the chunk boundary.
    for offset in 58..=66 {
        let prefix = ascii_pad(offset);
        let input = format!("{}X\u{1D15E}Y\u{2F800}Z", prefix);
        assert_all_forms_match_icu(&input);
    }
}

// ===========================================================================
// 6. ICU4X cross-validation (comprehensive)
// ===========================================================================

#[test]
fn cross_validate_supplementary_normalization_roundtrips() {
    // For each supplementary test character, verify the full roundtrip:
    // input -> NFD -> NFC should equal input -> NFC
    // input -> NFKD -> NFKC should equal input -> NFKC
    let test_chars = [
        "\u{1D15E}", // Musical half note
        "\u{1D15F}", // Musical quarter note
        "\u{1D160}", // Musical eighth note
        "\u{2F800}", // CJK compat
        "\u{2F801}", // CJK compat
        "\u{1D400}", // Math bold A
        "\u{1D41A}", // Math bold a
        "\u{10400}", // Deseret capital
        "\u{10428}", // Deseret small
        "\u{1F600}", // Emoji (no decomposition, stable)
    ];

    for input in &test_chars {
        // NFD -> NFC roundtrip
        let nfd_result = input.nfd();
        let nfd_then_nfc = nfd_result.nfc();
        let direct_nfc = input.nfc();
        assert_eq!(
            &*nfd_then_nfc, &*direct_nfc,
            "NFD->NFC roundtrip mismatch for {:?}",
            input
        );
        // Cross-validate with ICU4X
        let icu_nfd_then_nfc = icu_nfc(&icu_nfd(input));
        assert_eq!(
            &*nfd_then_nfc, &*icu_nfd_then_nfc,
            "NFD->NFC roundtrip ICU4X mismatch for {:?}",
            input
        );

        // NFKD -> NFKC roundtrip
        let nfkd_result = input.nfkd();
        let nfkd_then_nfkc = nfkd_result.nfkc();
        let direct_nfkc = input.nfkc();
        assert_eq!(
            &*nfkd_then_nfkc, &*direct_nfkc,
            "NFKD->NFKC roundtrip mismatch for {:?}",
            input
        );
        let icu_nfkd_then_nfkc = icu_nfkc(&icu_nfkd(input));
        assert_eq!(
            &*nfkd_then_nfkc, &*icu_nfkd_then_nfkc,
            "NFKD->NFKC roundtrip ICU4X mismatch for {:?}",
            input
        );
    }
}

#[test]
fn cross_validate_supplementary_idempotence() {
    // Normalization should be idempotent: normalizing the output again
    // should return the same string.
    let test_chars = [
        "\u{1D15E}", "\u{1D15F}", "\u{1D160}",
        "\u{2F800}", "\u{2F801}", "\u{2F804}",
        "\u{1D400}", "\u{1D41A}", "\u{1D504}",
        "\u{10400}", "\u{10428}",
    ];

    for input in &test_chars {
        let nfc1 = input.nfc();
        let nfc2 = nfc1.nfc();
        assert_eq!(&*nfc1, &*nfc2, "NFC not idempotent for {:?}", input);

        let nfd1 = input.nfd();
        let nfd2 = nfd1.nfd();
        assert_eq!(&*nfd1, &*nfd2, "NFD not idempotent for {:?}", input);

        let nfkc1 = input.nfkc();
        let nfkc2 = nfkc1.nfkc();
        assert_eq!(&*nfkc1, &*nfkc2, "NFKC not idempotent for {:?}", input);

        let nfkd1 = input.nfkd();
        let nfkd2 = nfkd1.nfkd();
        assert_eq!(&*nfkd1, &*nfkd2, "NFKD not idempotent for {:?}", input);
    }
}

#[test]
fn cross_validate_nfc_subset_nfkc() {
    // For supplementary chars: NFKC(x) == NFKC(NFC(x))
    // i.e., NFC does not interfere with further NFKC normalization.
    let test_inputs = [
        "\u{1D15E}", "\u{2F800}", "\u{1D400}", "\u{10400}",
        "hello \u{1D15E} world",
        "\u{1D400}\u{1D41A}\u{1D504}",
    ];

    for input in &test_inputs {
        let nfkc_direct = input.nfkc();
        let nfc_result = input.nfc();
        let nfc_then_nfkc = nfc_result.nfkc();
        assert_eq!(
            &*nfkc_direct, &*nfc_then_nfkc,
            "NFKC(x) != NFKC(NFC(x)) for {:?}",
            input
        );

        // Cross-validate
        let icu_nfkc_direct = icu_nfkc(input);
        assert_eq!(
            &*nfkc_direct, &*icu_nfkc_direct,
            "NFKC ICU4X mismatch for {:?}",
            input
        );
    }
}

// ===========================================================================
// proptest: supplementary plane fuzz testing
// ===========================================================================

#[cfg(test)]
mod proptest_supplementary {
    use super::*;
    use proptest::prelude::*;

    /// Strategy that generates strings containing supplementary plane characters
    /// mixed with ASCII, to stress-test SIMD boundary handling.
    fn supplementary_string_strategy() -> impl Strategy<Value = String> {
        // Mix of: ASCII, musical symbols, CJK compat, math alphanumeric, Deseret, emoji
        let char_strategy = prop_oneof![
            4 => (0x20u32..=0x7Eu32).prop_map(|c| char::from_u32(c).unwrap()),
            1 => prop::sample::select(vec![
                '\u{1D15E}', '\u{1D15F}', '\u{1D160}',     // Musical
                '\u{2F800}', '\u{2F801}', '\u{2F804}',      // CJK compat
                '\u{1D400}', '\u{1D41A}', '\u{1D434}',      // Math bold/italic
                '\u{1D504}', '\u{1D538}', '\u{1D7CE}',      // Math fraktur/doublestruck/digit
                '\u{10400}', '\u{10428}',                     // Deseret
                '\u{1F600}', '\u{1F60A}',                     // Emoji (stable)
            ]),
        ];

        proptest::collection::vec(char_strategy, 1..=64)
            .prop_map(|chars| chars.into_iter().collect::<String>())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(500))]

        #[test]
        fn supplementary_nfc_matches_icu4x(input in supplementary_string_strategy()) {
            let simd_result = input.nfc();
            let icu_result = icu_nfc(&input);
            prop_assert_eq!(&*simd_result, &*icu_result, "NFC mismatch");
        }

        #[test]
        fn supplementary_nfd_matches_icu4x(input in supplementary_string_strategy()) {
            let simd_result = input.nfd();
            let icu_result = icu_nfd(&input);
            prop_assert_eq!(&*simd_result, &*icu_result, "NFD mismatch");
        }

        #[test]
        fn supplementary_nfkc_matches_icu4x(input in supplementary_string_strategy()) {
            let simd_result = input.nfkc();
            let icu_result = icu_nfkc(&input);
            prop_assert_eq!(&*simd_result, &*icu_result, "NFKC mismatch");
        }

        #[test]
        fn supplementary_nfkd_matches_icu4x(input in supplementary_string_strategy()) {
            let simd_result = input.nfkd();
            let icu_result = icu_nfkd(&input);
            prop_assert_eq!(&*simd_result, &*icu_result, "NFKD mismatch");
        }

        #[test]
        fn supplementary_idempotent(input in supplementary_string_strategy()) {
            let nfc1 = input.nfc();
            let nfc2 = nfc1.nfc();
            prop_assert_eq!(&*nfc1, &*nfc2, "NFC not idempotent");

            let nfd1 = input.nfd();
            let nfd2 = nfd1.nfd();
            prop_assert_eq!(&*nfd1, &*nfd2, "NFD not idempotent");

            let nfkc1 = input.nfkc();
            let nfkc2 = nfkc1.nfkc();
            prop_assert_eq!(&*nfkc1, &*nfkc2, "NFKC not idempotent");

            let nfkd1 = input.nfkd();
            let nfkd2 = nfkd1.nfkd();
            prop_assert_eq!(&*nfkd1, &*nfkd2, "NFKD not idempotent");
        }
    }
}
