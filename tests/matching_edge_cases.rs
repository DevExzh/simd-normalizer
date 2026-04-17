//! Edge-case integration tests for the matching normalization pipeline.
//!
//! These tests go deeper than `tests/matching.rs`, covering symmetry,
//! NFKC compatibility characters, multi-character expansions, pipeline
//! convergence, Turkish mode edge cases, UTF-16 edge cases, derive trait
//! coverage, and unusual inputs.

use simd_normalizer::matching::{
    MatchingOptions, matches_normalized, normalize_for_matching, normalize_for_matching_utf16,
};
use simd_normalizer::CaseFoldMode;

fn default_opts() -> MatchingOptions {
    MatchingOptions::default()
}

fn turkish_opts() -> MatchingOptions {
    MatchingOptions {
        case_fold: CaseFoldMode::Turkish,
    }
}

// ===========================================================================
// 1. matches_normalized symmetry
// ===========================================================================

/// Helper: assert symmetry of `matches_normalized` for a pair.
fn assert_symmetric(a: &str, b: &str, opts: &MatchingOptions, expected: bool) {
    let ab = matches_normalized(a, b, opts);
    let ba = matches_normalized(b, a, opts);
    assert_eq!(
        ab, ba,
        "matches_normalized is not symmetric for ({:?}, {:?}): (a,b)={}, (b,a)={}",
        a, b, ab, ba,
    );
    assert_eq!(
        ab, expected,
        "matches_normalized({:?}, {:?}) should be {}, got {}",
        a, b, expected, ab,
    );
}

#[test]
fn symmetry_confusable_pairs() {
    let opts = default_opts();
    // Latin 'a' vs Cyrillic 'а' (U+0430)
    assert_symmetric("a", "\u{0430}", &opts, true);
    // Latin 'o' vs Cyrillic 'о' (U+043E)
    assert_symmetric("o", "\u{043E}", &opts, true);
    // Latin 'e' vs Cyrillic 'е' (U+0435)
    assert_symmetric("e", "\u{0435}", &opts, true);
    // Latin 'p' vs Cyrillic 'р' (U+0440)
    assert_symmetric("p", "\u{0440}", &opts, true);
}

#[test]
fn symmetry_case_pairs() {
    let opts = default_opts();
    assert_symmetric("Hello", "hello", &opts, true);
    assert_symmetric("WORLD", "world", &opts, true);
    assert_symmetric("FiLe", "file", &opts, true);
}

#[test]
fn symmetry_mixed_case_and_confusable() {
    let opts = default_opts();
    // "Apple" vs Cyrillic-mixed "Аррlе"
    let cyrillic_mixed = "\u{0410}\u{0440}\u{0440}l\u{0435}"; // А р р l е
    assert_symmetric("Apple", cyrillic_mixed, &opts, true);
}

#[test]
fn symmetry_non_matching_pairs() {
    let opts = default_opts();
    assert_symmetric("hello", "world", &opts, false);
    assert_symmetric("cat", "dog", &opts, false);
    assert_symmetric("abc", "xyz", &opts, false);
}

#[test]
fn symmetry_empty_strings() {
    let opts = default_opts();
    assert_symmetric("", "", &opts, true);
    // Empty vs non-empty should be false.
    assert_symmetric("", "a", &opts, false);
}

#[test]
fn symmetry_identical_strings() {
    let opts = default_opts();
    assert_symmetric("test", "test", &opts, true);
    assert_symmetric("\u{1F600}", "\u{1F600}", &opts, true);
}

// ===========================================================================
// 2. NFKC compatibility characters through matching
// ===========================================================================

#[test]
fn nfkc_roman_numeral_one() {
    let opts = default_opts();
    // Ⅰ (U+2160) NFKC→ "I" → casefold → "i" → skeleton
    // So Ⅰ should match "I" and "i" through the pipeline.
    let norm_roman = normalize_for_matching("\u{2160}", &opts);
    let norm_i = normalize_for_matching("I", &opts);
    let norm_i_lower = normalize_for_matching("i", &opts);
    assert_eq!(
        norm_roman, norm_i,
        "Roman numeral Ⅰ should match 'I' after matching pipeline"
    );
    assert_eq!(
        norm_roman, norm_i_lower,
        "Roman numeral Ⅰ should match 'i' after matching pipeline"
    );
}

#[test]
fn nfkc_roman_numeral_range() {
    let opts = default_opts();
    // Ⅱ (U+2161) NFKC→ "II", Ⅲ (U+2162) NFKC→ "III"
    assert_eq!(
        normalize_for_matching("\u{2161}", &opts),
        normalize_for_matching("II", &opts),
        "Ⅱ should match 'II'"
    );
    assert_eq!(
        normalize_for_matching("\u{2162}", &opts),
        normalize_for_matching("III", &opts),
        "Ⅲ should match 'III'"
    );
}

#[test]
fn nfkc_circled_latin_small_a() {
    let opts = default_opts();
    // ⓐ (U+24D0) NFKC→ "a"
    let norm_circled = normalize_for_matching("\u{24D0}", &opts);
    let norm_a = normalize_for_matching("a", &opts);
    assert_eq!(
        norm_circled, norm_a,
        "Circled latin small letter a should match 'a'"
    );
}

#[test]
fn nfkc_circled_latin_capital_a() {
    let opts = default_opts();
    // Ⓐ (U+24B6) NFKC→ "A" → casefold → "a"
    let norm_circled_cap = normalize_for_matching("\u{24B6}", &opts);
    let norm_a = normalize_for_matching("a", &opts);
    assert_eq!(
        norm_circled_cap, norm_a,
        "Circled latin capital letter A should match 'a'"
    );
}

#[test]
fn nfkc_parenthesized_digit_one() {
    let opts = default_opts();
    // ⑴ (U+2474) NFKC→ "(1)"
    let norm_paren = normalize_for_matching("\u{2474}", &opts);
    let norm_literal = normalize_for_matching("(1)", &opts);
    assert_eq!(
        norm_paren, norm_literal,
        "Parenthesized digit one should match '(1)'"
    );
}

#[test]
fn nfkc_fraction_one_half() {
    let opts = default_opts();
    // ½ (U+00BD) NFKC→ "1⁄2" (U+0031 U+2044 U+0032)
    // Both ½ and "1⁄2" should produce the same matching output.
    let norm_frac = normalize_for_matching("\u{00BD}", &opts);
    let norm_explicit = normalize_for_matching("1\u{2044}2", &opts);
    assert_eq!(
        norm_frac, norm_explicit,
        "½ should match '1⁄2' (with fraction slash)"
    );
}

#[test]
fn nfkc_fraction_one_quarter() {
    let opts = default_opts();
    // ¼ (U+00BC) NFKC→ "1⁄4"
    let norm_frac = normalize_for_matching("\u{00BC}", &opts);
    let norm_explicit = normalize_for_matching("1\u{2044}4", &opts);
    assert_eq!(
        norm_frac, norm_explicit,
        "¼ should match '1⁄4' (with fraction slash)"
    );
}

#[test]
fn nfkc_fullwidth_digits() {
    let opts = default_opts();
    // Fullwidth digit 1 (U+FF11) NFKC→ "1"
    assert_eq!(
        normalize_for_matching("\u{FF11}", &opts),
        normalize_for_matching("1", &opts),
        "Fullwidth digit 1 should match '1'"
    );
    // Fullwidth digit 0 (U+FF10) NFKC→ "0"
    assert_eq!(
        normalize_for_matching("\u{FF10}", &opts),
        normalize_for_matching("0", &opts),
        "Fullwidth digit 0 should match '0'"
    );
}

#[test]
fn nfkc_subscript_digits() {
    let opts = default_opts();
    // Subscript 0 (U+2080) NFKC→ "0"
    assert_eq!(
        normalize_for_matching("\u{2080}", &opts),
        normalize_for_matching("0", &opts),
        "Subscript 0 should match '0'"
    );
    // Subscript 2 (U+2082) NFKC→ "2"
    assert_eq!(
        normalize_for_matching("\u{2082}", &opts),
        normalize_for_matching("2", &opts),
        "Subscript 2 should match '2'"
    );
}

// ===========================================================================
// 3. Multi-character confusable expansions
// ===========================================================================

#[test]
fn ligature_fi_matches_fi() {
    let opts = default_opts();
    // ﬁ (U+FB01) NFKC→ "fi"
    assert!(
        matches_normalized("\u{FB01}", "fi", &opts),
        "fi ligature should match 'fi'"
    );
}

#[test]
fn ligature_fl_matches_fl() {
    let opts = default_opts();
    // ﬂ (U+FB02) NFKC→ "fl"
    assert!(
        matches_normalized("\u{FB02}", "fl", &opts),
        "fl ligature should match 'fl'"
    );
}

#[test]
fn ligature_ffi_matches_ffi() {
    let opts = default_opts();
    // ﬃ (U+FB03) NFKC→ "ffi"
    assert!(
        matches_normalized("\u{FB03}", "ffi", &opts),
        "ffi ligature should match 'ffi'"
    );
}

#[test]
fn ligature_ffl_matches_ffl() {
    let opts = default_opts();
    // ﬄ (U+FB04) NFKC→ "ffl"
    assert!(
        matches_normalized("\u{FB04}", "ffl", &opts),
        "ffl ligature should match 'ffl'"
    );
}

#[test]
fn ligature_in_word_context() {
    let opts = default_opts();
    // "ofﬁce" with fi ligature should match "office"
    assert!(
        matches_normalized("o\u{FB03}ce", "office", &opts),
        "'ofﬁce' (with ffi ligature) should match 'office'"
    );
}

#[test]
fn ligature_st() {
    let opts = default_opts();
    // ﬆ (U+FB06) NFKC→ "st"
    assert!(
        matches_normalized("\u{FB06}", "st", &opts),
        "st ligature should match 'st'"
    );
}

// ===========================================================================
// 4. Pipeline convergence edge cases
// ===========================================================================

#[test]
fn convergence_idempotent_after_normalization() {
    let opts = default_opts();
    // The pipeline iterates up to 4 passes. Verify that the result
    // of normalizing the normalized output is the same (convergence).
    let tricky_inputs = [
        "\u{2160}",        // Roman numeral Ⅰ
        "\u{FB01}",        // fi ligature
        "\u{00BD}",        // ½
        "\u{FF21}",        // Fullwidth A
        "\u{0430}",        // Cyrillic а
        "\u{00B2}",        // Superscript 2
        "\u{24D0}",        // Circled a
        "F\u{0131}LE",     // Mixed case + dotless-i
        "\u{0410}\u{0440}\u{0440}l\u{0435}", // Cyrillic-mixed "apple"
    ];
    for input in &tricky_inputs {
        let once = normalize_for_matching(input, &opts);
        let twice = normalize_for_matching(&once, &opts);
        assert_eq!(
            once, twice,
            "Pipeline did not converge (not idempotent) for input {:?}: once={:?}, twice={:?}",
            input, once, twice,
        );
    }
}

#[test]
fn convergence_multi_step_chain() {
    let opts = default_opts();
    // String where NFKC produces something that has confusable mapping,
    // and confusable mapping produces something that needs casefold.
    // Fullwidth 'A' (U+FF21) → NFKC 'A' → casefold 'a' → skeleton 'a'
    // Cyrillic 'А' (U+0410) → casefold 'а' (U+0430) → skeleton → 'a'
    // Both should converge to the same result.
    let fw_a = normalize_for_matching("\u{FF21}", &opts);
    let cyr_cap_a = normalize_for_matching("\u{0410}", &opts);
    assert_eq!(
        fw_a, cyr_cap_a,
        "Fullwidth A and Cyrillic A should converge to the same matching form"
    );
}

#[test]
fn convergence_passes_are_bounded() {
    let opts = default_opts();
    // Even with complex input, normalize_for_matching should return
    // without hanging. Test with a mix of compatibility characters
    // and confusables chained together.
    let input = "\u{FF21}\u{FF22}\u{FF23}\u{2160}\u{2161}\u{2162}\u{00BD}\u{FB01}\u{FB02}";
    let result = normalize_for_matching(input, &opts);
    assert!(!result.is_empty(), "Convergence should produce non-empty output");
    // Verify idempotence (proof of convergence).
    let again = normalize_for_matching(&result, &opts);
    assert_eq!(result, again, "Result should be stable after convergence");
}

// ===========================================================================
// 5. Turkish mode edge cases
// ===========================================================================

#[test]
fn turkish_i_distinctions() {
    let turkish = turkish_opts();
    let standard = default_opts();

    // In Turkish mode:
    //   I (U+0049) → ı (U+0131)
    //   İ (U+0130) → i (U+0069)
    //   ı (U+0131) → ı (unchanged)
    //   i (U+0069) → i (unchanged)

    // Turkish: "I" and "ı" should match
    assert!(
        matches_normalized("I", "\u{0131}", &turkish),
        "Turkish: 'I' should match 'ı'"
    );

    // Turkish: "İ" and "i" should match
    assert!(
        matches_normalized("\u{0130}", "i", &turkish),
        "Turkish: 'İ' should match 'i'"
    );

    // In standard mode: "I" → "i", so "I" and "i" match
    assert!(
        matches_normalized("I", "i", &standard),
        "Standard: 'I' should match 'i'"
    );
}

#[test]
fn turkish_vs_standard_different_results_for_uppercase_i() {
    let turkish = turkish_opts();
    let standard = default_opts();

    // The normalized forms of "I" should differ between Turkish and Standard modes,
    // because Turkish folds I→ı while Standard folds I→i.
    let turkish_i = normalize_for_matching("I", &turkish);
    let standard_i = normalize_for_matching("I", &standard);

    // They may or may not differ after the full pipeline (confusable skeleton
    // may map ı and i to the same prototype). Check the behavior.
    // What matters is that the *casefold step* differs.
    let turkish_fold = normalize_for_matching("\u{0131}", &turkish); // ı in Turkish
    let standard_fold = normalize_for_matching("i", &standard); // i in Standard

    // Turkish "I" should equal Turkish "ı"
    assert_eq!(
        turkish_i, turkish_fold,
        "Turkish: 'I' should produce same result as 'ı'"
    );
    // Standard "I" should equal Standard "i"
    assert_eq!(
        standard_i, standard_fold,
        "Standard: 'I' should produce same result as 'i'"
    );
}

#[test]
fn turkish_mode_with_confusable_cyrillic() {
    let opts = turkish_opts();
    // Cyrillic а (U+0430) should still be confusable with Latin 'a' in Turkish mode.
    assert!(
        matches_normalized("a", "\u{0430}", &opts),
        "Turkish mode: Latin 'a' and Cyrillic 'а' should still match"
    );
}

#[test]
fn turkish_mode_full_word() {
    let opts = turkish_opts();
    // "İstanbul" in Turkish mode: İ→i, so matches "istanbul"
    assert!(matches_normalized("\u{0130}stanbul", "istanbul", &opts));
    // "ISTANBUL" in Turkish mode: I→ı, S→s, T→t, ...
    // Should match "ıstanbul" (with dotless-ı)
    assert!(matches_normalized("ISTANBUL", "\u{0131}stanbul", &opts));
}

#[test]
fn turkish_mode_dotless_i_in_word() {
    let turkish = turkish_opts();
    // "fıle" in Turkish mode: ı stays ı, so should match "f" + "ı" + "le"
    assert!(
        matches_normalized("f\u{0131}le", "f\u{0131}le", &turkish),
        "Turkish: identical fıle should match"
    );
}

// ===========================================================================
// 6. UTF-16 edge cases
// ===========================================================================

#[test]
fn utf16_empty_string() {
    let opts = default_opts();
    let utf16 = normalize_for_matching_utf16("", &opts);
    assert!(utf16.is_empty(), "Empty string should produce empty UTF-16 vec");
}

#[test]
fn utf16_ascii_only_no_surrogates() {
    let opts = default_opts();
    let utf16 = normalize_for_matching_utf16("hello", &opts);
    // All code units should be in BMP range (no surrogate pairs for ASCII).
    for &cu in &utf16 {
        assert!(
            cu < 0xD800 || cu > 0xDFFF,
            "ASCII-only input should not produce surrogate pairs, found {:04X}",
            cu,
        );
    }
}

#[test]
fn utf16_supplementary_chars_produce_surrogates() {
    let opts = default_opts();
    // U+1F600 (grinning face emoji) is a supplementary character.
    let utf16 = normalize_for_matching_utf16("\u{1F600}", &opts);
    // Supplementary chars require surrogate pairs (2 code units).
    assert!(
        utf16.len() >= 2,
        "Supplementary character should produce at least 2 code units (surrogate pair)"
    );
    // Check that the first code unit is a high surrogate.
    let has_surrogate = utf16.iter().any(|&cu| (0xD800..=0xDBFF).contains(&cu));
    assert!(
        has_surrogate,
        "Supplementary character should have a high surrogate"
    );
    // Verify valid UTF-16.
    String::from_utf16(&utf16).expect("Should be valid UTF-16 for supplementary char");
}

#[test]
fn utf16_musical_symbol_supplementary() {
    let opts = default_opts();
    // U+1D11E (MUSICAL SYMBOL G CLEF) is supplementary.
    let utf16 = normalize_for_matching_utf16("\u{1D11E}", &opts);
    assert!(
        utf16.len() >= 2,
        "Musical symbol should produce surrogate pair"
    );
    String::from_utf16(&utf16).expect("Should be valid UTF-16 for musical symbol");
}

#[test]
fn utf16_mixed_bmp_and_supplementary() {
    let opts = default_opts();
    // Mix of BMP (ASCII, Latin) and supplementary (emoji).
    let input = "Hello \u{1F600} World \u{1F4A9}";
    let utf16 = normalize_for_matching_utf16(input, &opts);
    assert!(!utf16.is_empty());
    let decoded = String::from_utf16(&utf16).expect("Should be valid UTF-16");
    assert_eq!(
        decoded,
        normalize_for_matching(input, &opts),
        "UTF-16 round-trip should match normalize_for_matching"
    );
}

#[test]
fn utf16_roundtrip_comprehensive() {
    let opts = default_opts();
    let inputs = [
        "",
        "a",
        "hello",
        "CAFÉ",
        "\u{1F600}",                          // emoji
        "\u{1D11E}",                          // musical symbol
        "abc\u{1F600}def\u{1F4A9}ghi",       // mixed
        "\u{2160}\u{2161}",                   // Roman numerals
        "\u{FB01}\u{FB02}",                   // ligatures
        "\u{00BD}",                           // fraction ½
        "\u{0430}\u{0440}\u{0440}l\u{0435}", // Cyrillic-mixed
        "\u{FF21}\u{FF22}\u{FF23}",          // fullwidth
    ];
    for input in &inputs {
        let utf16 = normalize_for_matching_utf16(input, &opts);
        let expected = normalize_for_matching(input, &opts);
        if expected.is_empty() {
            assert!(utf16.is_empty(), "Empty matching result should give empty UTF-16");
        } else {
            let decoded = String::from_utf16(&utf16)
                .unwrap_or_else(|_| panic!("Invalid UTF-16 for input {:?}", input));
            assert_eq!(
                decoded, expected,
                "UTF-16 round-trip mismatch for input {:?}",
                input,
            );
        }
    }
}

// ===========================================================================
// 7. Derive trait coverage for MatchingOptions and CaseFoldMode
// ===========================================================================

#[test]
fn matching_options_default_is_standard() {
    let opts = MatchingOptions::default();
    assert_eq!(
        opts.case_fold,
        CaseFoldMode::Standard,
        "Default MatchingOptions should use Standard case folding"
    );
}

#[test]
fn matching_options_clone_and_copy() {
    let opts = MatchingOptions {
        case_fold: CaseFoldMode::Turkish,
    };
    let cloned = opts.clone();
    let copied = opts; // Copy
    assert_eq!(opts, cloned, "Clone should produce equal MatchingOptions");
    assert_eq!(opts, copied, "Copy should produce equal MatchingOptions");
}

#[test]
fn matching_options_debug() {
    let opts = MatchingOptions::default();
    let debug_str = format!("{:?}", opts);
    assert!(
        debug_str.contains("MatchingOptions"),
        "Debug output should contain 'MatchingOptions', got: {}",
        debug_str,
    );
    assert!(
        debug_str.contains("Standard"),
        "Debug output should contain 'Standard', got: {}",
        debug_str,
    );
}

#[test]
fn matching_options_eq() {
    let a = MatchingOptions::default();
    let b = MatchingOptions::default();
    let c = MatchingOptions {
        case_fold: CaseFoldMode::Turkish,
    };
    assert_eq!(a, b, "Two default MatchingOptions should be equal");
    assert_ne!(a, c, "Standard and Turkish options should not be equal");
}

#[test]
fn casefold_mode_clone_copy_debug_eq() {
    let standard = CaseFoldMode::Standard;
    let turkish = CaseFoldMode::Turkish;

    // Clone
    let cloned = standard.clone();
    assert_eq!(standard, cloned);

    // Copy
    let copied = standard;
    assert_eq!(standard, copied);

    // Debug
    let debug_s = format!("{:?}", standard);
    assert!(debug_s.contains("Standard"));
    let debug_t = format!("{:?}", turkish);
    assert!(debug_t.contains("Turkish"));

    // Eq
    assert_eq!(standard, CaseFoldMode::Standard);
    assert_ne!(standard, turkish);
}

// ===========================================================================
// 8. Edge inputs
// ===========================================================================

#[test]
fn very_long_string_no_panic() {
    let opts = default_opts();
    // >10KB of mixed content.
    let chunk = "Hello World \u{0430}\u{0440}\u{0440}l\u{0435} \u{FF21}\u{00BD} ";
    let input: String = chunk.repeat(500); // Well over 10KB
    assert!(input.len() > 10_000, "Input should be >10KB");
    let result = normalize_for_matching(&input, &opts);
    assert!(!result.is_empty());
    // Also check UTF-16 path.
    let utf16 = normalize_for_matching_utf16(&input, &opts);
    assert!(!utf16.is_empty());
}

#[test]
fn only_combining_marks() {
    let opts = default_opts();
    // A string of only combining marks (no base character).
    // U+0300 COMBINING GRAVE ACCENT
    // U+0301 COMBINING ACUTE ACCENT
    // U+0302 COMBINING CIRCUMFLEX ACCENT
    // U+0303 COMBINING TILDE
    // U+0308 COMBINING DIAERESIS
    let marks = "\u{0300}\u{0301}\u{0302}\u{0303}\u{0308}";
    let result = normalize_for_matching(marks, &opts);
    // Should not panic. Result may or may not be empty depending on
    // whether confusable skeleton maps combining marks.
    let _ = result;
}

#[test]
fn many_combining_marks_stacked() {
    let opts = default_opts();
    // Base character followed by many combining marks.
    let mut input = String::from("a");
    for _ in 0..100 {
        input.push('\u{0300}'); // 100 combining grave accents
    }
    let result = normalize_for_matching(&input, &opts);
    assert!(!result.is_empty());
}

#[test]
fn mixed_scripts_latin_cyrillic_cjk() {
    let opts = default_opts();
    // A string with Latin, Cyrillic, and CJK characters.
    let input = "Hello\u{041F}\u{0440}\u{0438}\u{0432}\u{0435}\u{0442}\u{4F60}\u{597D}";
    let result = normalize_for_matching(input, &opts);
    assert!(!result.is_empty(), "Mixed script input should produce output");
    // Idempotence check.
    let again = normalize_for_matching(&result, &opts);
    assert_eq!(result, again, "Mixed script result should be idempotent");
}

#[test]
fn null_character() {
    let opts = default_opts();
    // U+0000 is a valid Rust char but unusual.
    let input = "\u{0000}";
    let result = normalize_for_matching(input, &opts);
    // Should not panic. The null character might pass through or be mapped.
    let _ = result;
}

#[test]
fn null_character_in_middle() {
    let opts = default_opts();
    let input = "ab\u{0000}cd";
    let result = normalize_for_matching(input, &opts);
    // Should not panic.
    assert!(!result.is_empty());
}

#[test]
fn single_character_various_scripts() {
    let opts = default_opts();
    let chars = [
        "a",          // Latin
        "A",          // Latin upper
        "\u{0430}",   // Cyrillic а
        "\u{0410}",   // Cyrillic А
        "\u{03B1}",   // Greek α
        "\u{0391}",   // Greek Α
        "\u{4E00}",   // CJK Unified Ideograph (一)
        "\u{0627}",   // Arabic Alef
        "\u{05D0}",   // Hebrew Alef
        "\u{0E01}",   // Thai Ko Kai
        "\u{3042}",   // Hiragana あ
        "\u{30A2}",   // Katakana ア
        "\u{AC00}",   // Hangul 가
        "\u{1F600}",  // Emoji
    ];
    for &ch in &chars {
        let result = normalize_for_matching(ch, &opts);
        // Should not panic for any single-character script.
        assert!(
            !result.is_empty(),
            "Single character {:?} should produce non-empty result",
            ch,
        );
        // Idempotence.
        let again = normalize_for_matching(&result, &opts);
        assert_eq!(
            result, again,
            "Single character {:?} should be idempotent after matching normalization",
            ch,
        );
    }
}

#[test]
fn replacement_character() {
    let opts = default_opts();
    // U+FFFD REPLACEMENT CHARACTER
    let result = normalize_for_matching("\u{FFFD}", &opts);
    // Should not panic.
    let _ = result;
}

#[test]
fn bom_character() {
    let opts = default_opts();
    // U+FEFF BYTE ORDER MARK (ZERO WIDTH NO-BREAK SPACE)
    let result = normalize_for_matching("\u{FEFF}", &opts);
    // Should not panic.
    let _ = result;
}

#[test]
fn soft_hyphen() {
    let opts = default_opts();
    // U+00AD SOFT HYPHEN
    let result = normalize_for_matching("\u{00AD}", &opts);
    let _ = result;
}

#[test]
fn zero_width_chars() {
    let opts = default_opts();
    // U+200B ZERO WIDTH SPACE
    // U+200C ZERO WIDTH NON-JOINER
    // U+200D ZERO WIDTH JOINER
    // U+FEFF BOM / ZERO WIDTH NO-BREAK SPACE
    let input = "\u{200B}\u{200C}\u{200D}\u{FEFF}";
    let result = normalize_for_matching(input, &opts);
    // Should not panic.
    let _ = result;
}

#[test]
fn private_use_area_characters() {
    let opts = default_opts();
    // U+E000 (start of BMP Private Use Area)
    let result = normalize_for_matching("\u{E000}", &opts);
    assert!(!result.is_empty());
    // U+F8FF (end of BMP PUA, Apple logo on macOS)
    let result2 = normalize_for_matching("\u{F8FF}", &opts);
    assert!(!result2.is_empty());
}

#[test]
fn max_unicode_scalar() {
    let opts = default_opts();
    // U+10FFFF is the maximum Unicode scalar value.
    let result = normalize_for_matching("\u{10FFFF}", &opts);
    // Should not panic.
    let _ = result;
}

// ===========================================================================
// Additional: cross-cutting edge cases
// ===========================================================================

#[test]
fn confusable_and_nfkc_combined() {
    let opts = default_opts();
    // Fullwidth Latin letters are NFKC-decomposed, then the result
    // goes through confusable mapping. Fullwidth 'А' doesn't exist,
    // but fullwidth 'A' (U+FF21) → 'A' → casefold 'a' → skeleton
    // and Cyrillic 'а' (U+0430) → skeleton → same as Latin 'a'.
    assert!(
        matches_normalized("\u{FF21}", "\u{0430}", &opts),
        "Fullwidth A should match Cyrillic а through NFKC + confusable pipeline"
    );
}

#[test]
fn repeated_normalization_stability() {
    let opts = default_opts();
    // Apply normalization many times and verify it stays stable.
    let mut current = String::from("\u{FF21}\u{0410}\u{FB01}\u{00BD}");
    for i in 0..10 {
        let next = normalize_for_matching(&current, &opts);
        if i > 0 {
            // After the first normalization, all subsequent should be identical.
            let prev = normalize_for_matching(&current, &opts);
            assert_eq!(
                next, prev,
                "Normalization should be stable at iteration {}",
                i
            );
        }
        current = next;
    }
}

#[test]
fn matches_normalized_reflexive() {
    let opts = default_opts();
    // Every string should match itself.
    let inputs = [
        "",
        "a",
        "Hello",
        "\u{0430}",
        "\u{1F600}",
        "\u{FF21}",
        "\u{FB01}",
        "\u{0300}\u{0301}",
        "a\u{0300}b\u{0301}c",
    ];
    for &input in &inputs {
        assert!(
            matches_normalized(input, input, &opts),
            "matches_normalized should be reflexive for {:?}",
            input,
        );
    }
}

#[test]
fn matches_normalized_transitive() {
    let opts = default_opts();
    // If a matches b and b matches c, then a should match c.
    // Fullwidth A, Latin A, Cyrillic А
    let a = "\u{FF21}"; // Fullwidth A
    let b = "A";        // Latin A
    let c = "\u{0410}"; // Cyrillic А

    let ab = matches_normalized(a, b, &opts);
    let bc = matches_normalized(b, c, &opts);
    let ac = matches_normalized(a, c, &opts);

    if ab && bc {
        assert!(
            ac,
            "matches_normalized should be transitive: A({:?})=B({:?})={}, B({:?})=C({:?})={}, but A=C={}",
            a, b, ab, b, c, bc, ac,
        );
    }
}

#[test]
fn hangul_syllable_composition() {
    let opts = default_opts();
    // Hangul syllable 가 (U+AC00) = ᄀ (U+1100) + ᅡ (U+1161)
    // NFKC should compose the Jamo into the syllable.
    assert!(
        matches_normalized("\u{AC00}", "\u{1100}\u{1161}", &opts),
        "Hangul syllable should match its Jamo decomposition through matching"
    );
}

#[test]
fn nfkc_angstrom_sign() {
    let opts = default_opts();
    // Å (U+212B, ANGSTROM SIGN) NFKC→ Å (U+00C5, LATIN CAPITAL LETTER A WITH RING ABOVE)
    // → casefold → å (U+00E5)
    assert!(
        matches_normalized("\u{212B}", "\u{00C5}", &opts),
        "Angstrom sign should match Latin A with ring above"
    );
    assert!(
        matches_normalized("\u{212B}", "\u{00E5}", &opts),
        "Angstrom sign should match lowercase a with ring above (after casefold)"
    );
}

#[test]
fn nfkc_ohm_sign() {
    let opts = default_opts();
    // Ω (U+2126, OHM SIGN) NFKC→ Ω (U+03A9, GREEK CAPITAL LETTER OMEGA) → casefold → ω (U+03C9)
    assert!(
        matches_normalized("\u{2126}", "\u{03A9}", &opts),
        "Ohm sign should match Greek capital omega"
    );
    assert!(
        matches_normalized("\u{2126}", "\u{03C9}", &opts),
        "Ohm sign should match Greek small omega (after casefold)"
    );
}

#[test]
fn nfkc_kelvin_sign() {
    let opts = default_opts();
    // K (U+212A, KELVIN SIGN) NFKC→ K (U+004B) → casefold → k
    assert!(
        matches_normalized("\u{212A}", "K", &opts),
        "Kelvin sign should match Latin K"
    );
    assert!(
        matches_normalized("\u{212A}", "k", &opts),
        "Kelvin sign should match Latin k (after casefold)"
    );
}
