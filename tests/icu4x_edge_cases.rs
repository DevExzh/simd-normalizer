// tests/icu4x_edge_cases.rs
//
// Edge case tests ported from ICU4X's normalizer test suite
// (3rdparty/icu4x/components/normalizer/tests/tests.rs).
//
// Each test verifies simd-normalizer output and cross-validates against
// icu_normalizer as a reference implementation.

use icu_normalizer::{ComposingNormalizerBorrowed, DecomposingNormalizerBorrowed};
use simd_normalizer::UnicodeNormalization;

// ---------------------------------------------------------------------------
// Helper: assert simd-normalizer matches expected AND matches icu_normalizer
// ---------------------------------------------------------------------------

fn assert_nfd(input: &str, expected: &str) {
    let simd_result = simd_normalizer::nfd().normalize(input);
    let icu_result = DecomposingNormalizerBorrowed::new_nfd().normalize(input);
    assert_eq!(
        &*simd_result, expected,
        "NFD mismatch for {:?}: simd produced {:?}, expected {:?}",
        input, simd_result, expected
    );
    assert_eq!(
        &*simd_result, &*icu_result,
        "NFD cross-validation failed for {:?}: simd={:?}, icu={:?}",
        input, simd_result, icu_result
    );
}

fn assert_nfkd(input: &str, expected: &str) {
    let simd_result = simd_normalizer::nfkd().normalize(input);
    let icu_result = DecomposingNormalizerBorrowed::new_nfkd().normalize(input);
    assert_eq!(
        &*simd_result, expected,
        "NFKD mismatch for {:?}: simd produced {:?}, expected {:?}",
        input, simd_result, expected
    );
    assert_eq!(
        &*simd_result, &*icu_result,
        "NFKD cross-validation failed for {:?}: simd={:?}, icu={:?}",
        input, simd_result, icu_result
    );
}

fn assert_nfc(input: &str, expected: &str) {
    let simd_result = simd_normalizer::nfc().normalize(input);
    let icu_result = ComposingNormalizerBorrowed::new_nfc().normalize(input);
    assert_eq!(
        &*simd_result, expected,
        "NFC mismatch for {:?}: simd produced {:?}, expected {:?}",
        input, simd_result, expected
    );
    assert_eq!(
        &*simd_result, &*icu_result,
        "NFC cross-validation failed for {:?}: simd={:?}, icu={:?}",
        input, simd_result, icu_result
    );
}

fn assert_nfkc(input: &str, expected: &str) {
    let simd_result = simd_normalizer::nfkc().normalize(input);
    let icu_result = ComposingNormalizerBorrowed::new_nfkc().normalize(input);
    assert_eq!(
        &*simd_result, expected,
        "NFKC mismatch for {:?}: simd produced {:?}, expected {:?}",
        input, simd_result, expected
    );
    assert_eq!(
        &*simd_result, &*icu_result,
        "NFKC cross-validation failed for {:?}: simd={:?}, icu={:?}",
        input, simd_result, icu_result
    );
}

// ---------------------------------------------------------------------------
// 1. NFD edge cases
// ---------------------------------------------------------------------------

#[test]
fn test_nfd_edge_cases() {
    // Basic umlaut decomposition
    assert_nfd("\u{00E4}", "a\u{0308}");
    assert_nfd("\u{00C4}", "A\u{0308}");

    // Vietnamese multi-level decomposition
    assert_nfd("\u{1EC7}", "e\u{0323}\u{0302}");
    assert_nfd("\u{1EC6}", "E\u{0323}\u{0302}");

    // Musical note: composition exclusion, decomposes but never recomposes
    assert_nfd("𝅗𝅥", "𝅗\u{1D165}");

    // Ohm sign (U+2126) canonically decomposes to Greek capital Omega (U+03A9)
    assert_nfd("\u{2126}", "\u{03A9}");

    // Half-width katakana + dakuten/handakuten: UNCHANGED in NFD
    // (compatibility mappings are not applied in canonical decomposition)
    assert_nfd("\u{FF8D}\u{FF9E}", "\u{FF8D}\u{FF9E}");
    assert_nfd("\u{FF8D}\u{FF9F}", "\u{FF8D}\u{FF9F}");

    // Latin ligature fi: UNCHANGED in NFD (compatibility mapping only)
    assert_nfd("\u{FB01}", "\u{FB01}");

    // Arabic ligature Sallallahou Alayhe Wasallam: UNCHANGED in NFD
    assert_nfd("\u{FDFA}", "\u{FDFA}");

    // Parenthesized Hangul Kiyeok A: UNCHANGED in NFD
    assert_nfd("\u{320E}", "\u{320E}");

    // Iota subscript: UNCHANGED (it is already a combining mark)
    assert_nfd("\u{0345}", "\u{0345}");
}

// ---------------------------------------------------------------------------
// 2. NFKD edge cases (compatibility decomposition)
// ---------------------------------------------------------------------------

#[test]
fn test_nfkd_edge_cases() {
    // Same base chars get canonical decomposition as well
    assert_nfkd("\u{00E4}", "a\u{0308}");
    assert_nfkd("\u{00C4}", "A\u{0308}");
    assert_nfkd("\u{1EC7}", "e\u{0323}\u{0302}");
    assert_nfkd("\u{1EC6}", "E\u{0323}\u{0302}");
    assert_nfkd("𝅗𝅥", "𝅗\u{1D165}");
    assert_nfkd("\u{2126}", "\u{03A9}");

    // Half-width katakana + dakuten -> full-width katakana + combining dakuten
    assert_nfkd("\u{FF8D}\u{FF9E}", "\u{30D8}\u{3099}");

    // Half-width katakana + handakuten -> full-width katakana + combining handakuten
    assert_nfkd("\u{FF8D}\u{FF9F}", "\u{30D8}\u{309A}");

    // Latin ligature fi -> "fi"
    assert_nfkd("\u{FB01}", "fi");

    // Arabic ligature FDFA -> 18-char expansion (longest NFKD in Unicode)
    assert_nfkd(
        "\u{FDFA}",
        "\u{0635}\u{0644}\u{0649} \u{0627}\u{0644}\u{0644}\u{0647} \u{0639}\u{0644}\u{064A}\u{0647} \u{0648}\u{0633}\u{0644}\u{0645}",
    );

    // Parenthesized Hangul -> decomposed Hangul in parentheses
    assert_nfkd("\u{320E}", "(\u{1100}\u{1161})");

    // Iota subscript: UNCHANGED in NFKD too
    assert_nfkd("\u{0345}", "\u{0345}");
}

// ---------------------------------------------------------------------------
// 3. NFC edge cases (canonical composition)
// ---------------------------------------------------------------------------

#[test]
fn test_nfc_composition_exclusion() {
    // Combining sequences compose into precomposed characters
    assert_nfc("a\u{0308}", "\u{00E4}");
    assert_nfc("A\u{0308}", "\u{00C4}");

    // Multi-level composition: e + combining dot below + combining circumflex
    assert_nfc("e\u{0323}\u{0302}", "\u{1EC7}");
    assert_nfc("E\u{0323}\u{0302}", "\u{1EC6}");

    // Musical note: composition exclusion -- does NOT compose back
    assert_nfc("𝅗𝅥", "𝅗\u{1D165}");

    // Ohm sign composes to Omega (via decomposition then identity in NFC)
    assert_nfc("\u{2126}", "\u{03A9}");

    // Half-width katakana: UNCHANGED in NFC (no canonical mapping)
    assert_nfc("\u{FF8D}\u{FF9E}", "\u{FF8D}\u{FF9E}");
    assert_nfc("\u{FF8D}\u{FF9F}", "\u{FF8D}\u{FF9F}");

    // Latin ligature: UNCHANGED in NFC
    assert_nfc("\u{FB01}", "\u{FB01}");

    // Arabic ligature: UNCHANGED in NFC
    assert_nfc("\u{FDFA}", "\u{FDFA}");

    // Parenthesized Hangul: UNCHANGED in NFC
    assert_nfc("\u{320E}", "\u{320E}");

    // Iota subscript: UNCHANGED in NFC
    assert_nfc("\u{0345}", "\u{0345}");
}

// ---------------------------------------------------------------------------
// 4. NFKC edge cases (compatibility composition)
// ---------------------------------------------------------------------------

#[test]
fn test_nfkc_compatibility_composition() {
    // Basic composition still works
    assert_nfkc("a\u{0308}", "\u{00E4}");
    assert_nfkc("A\u{0308}", "\u{00C4}");
    assert_nfkc("e\u{0323}\u{0302}", "\u{1EC7}");
    assert_nfkc("E\u{0323}\u{0302}", "\u{1EC6}");

    // Musical note: composition exclusion applies in NFKC too
    assert_nfkc("𝅗𝅥", "𝅗\u{1D165}");

    // Ohm sign
    assert_nfkc("\u{2126}", "\u{03A9}");

    // Half-width katakana + dakuten -> full-width composed (Be with dakuten)
    assert_nfkc("\u{FF8D}\u{FF9E}", "\u{30D9}");

    // Half-width katakana + handakuten -> full-width composed (Pe with handakuten)
    assert_nfkc("\u{FF8D}\u{FF9F}", "\u{30DA}");

    // Latin ligature fi -> "fi"
    assert_nfkc("\u{FB01}", "fi");

    // Arabic ligature FDFA -> full expansion (same as NFKD since no recomposition possible)
    assert_nfkc(
        "\u{FDFA}",
        "\u{0635}\u{0644}\u{0649} \u{0627}\u{0644}\u{0644}\u{0647} \u{0639}\u{0644}\u{064A}\u{0647} \u{0648}\u{0633}\u{0644}\u{0645}",
    );

    // Parenthesized Hangul -> expanded, Hangul partially recomposed
    assert_nfkc("\u{320E}", "(\u{AC00})");

    // Iota subscript: UNCHANGED in NFKC
    assert_nfkc("\u{0345}", "\u{0345}");
}

// ---------------------------------------------------------------------------
// 5. Accented digraph: CCC reorder after compatibility decomposition
// ---------------------------------------------------------------------------

#[test]
fn test_accented_digraph_ccc_reorder() {
    // U+01C4 (DZ with caron) decomposes in NFKD to D + Z + combining caron (U+030C).
    // When U+0323 (combining dot below, CCC=220) follows, canonical ordering
    // puts U+0323 (CCC=220) before U+030C (CCC=230).
    assert_nfkd("\u{01C4}\u{0323}", "DZ\u{0323}\u{030C}");

    // Same reordering when the combining marks are already present but in wrong order
    assert_nfkd("DZ\u{030C}\u{0323}", "DZ\u{0323}\u{030C}");

    // Cross-validate: NFC and NFD forms of the reordered sequence
    // In NFD, U+01C4 does NOT decompose (it is a compatibility mapping), so
    // the combining marks stay in input order after canonical reordering of
    // the two trailing marks.
    assert_nfd("\u{01C4}\u{0323}", "\u{01C4}\u{0323}");
    assert_nfd("DZ\u{030C}\u{0323}", "DZ\u{0323}\u{030C}");
}

// ---------------------------------------------------------------------------
// 6. Sinhala DDD edge case (interleaved decomposition)
// ---------------------------------------------------------------------------

#[test]
fn test_sinhala_ddd_decomposition() {
    // U+0DDD (Sinhala Kombuva Haa-Ahaasaya) decomposes to U+0DD9 + U+0DCA + U+0DCF
    // in a multi-step process. When followed by U+0334 (combining tilde overlay,
    // CCC=1), the decomposition interleaves as:
    //   U+0DD9 (CCC=0) + U+0DCF (CCC=0) + U+0334 (CCC=1) + U+0DCA (CCC=9)
    assert_nfd("\u{0DDD}\u{0334}", "\u{0DD9}\u{0DCF}\u{0334}\u{0DCA}");

    // Cross-validate against ICU4X for NFD
    let icu_nfd = DecomposingNormalizerBorrowed::new_nfd();
    assert_eq!(
        icu_nfd.normalize("\u{0DDD}\u{0334}"),
        "\u{0DD9}\u{0DCF}\u{0334}\u{0DCA}",
        "ICU4X NFD reference should match expected Sinhala decomposition"
    );

    // In NFC, U+0DDD is already a composed form and is NFC-stable, so
    // "\u{0DDD}\u{0334}" stays as-is (the combining tilde overlay just
    // follows the precomposed Sinhala character).
    assert_nfc("\u{0DDD}\u{0334}", "\u{0DDD}\u{0334}");
}

// ---------------------------------------------------------------------------
// 7. is_normalized form distinctions
// ---------------------------------------------------------------------------

#[test]
fn test_is_normalized_form_distinctions() {
    // -- Pure ASCII: normalized in all forms --
    let aaa = "aaa";
    assert!(aaa.is_nfc(), "'aaa' should be NFC");
    assert!(aaa.is_nfd(), "'aaa' should be NFD");
    assert!(aaa.is_nfkc(), "'aaa' should be NFKC");
    assert!(aaa.is_nfkd(), "'aaa' should be NFKD");
    // Cross-validate
    assert!(ComposingNormalizerBorrowed::new_nfc().is_normalized(aaa));
    assert!(DecomposingNormalizerBorrowed::new_nfd().is_normalized(aaa));
    assert!(ComposingNormalizerBorrowed::new_nfkc().is_normalized(aaa));
    assert!(DecomposingNormalizerBorrowed::new_nfkd().is_normalized(aaa));

    // -- Musical note composition exclusion: normalized in ALL forms --
    // The musical symbol half note (U+1D15E -> U+1D157 + U+1D165) is a composition
    // exclusion, so the decomposed form IS the NFC form too.
    let note = "a𝅗\u{1D165}a";
    assert!(note.is_nfc(), "note should be NFC (composition exclusion)");
    assert!(note.is_nfd(), "note should be NFD");
    assert!(note.is_nfkc(), "note should be NFKC");
    assert!(note.is_nfkd(), "note should be NFKD");
    // Cross-validate
    assert!(ComposingNormalizerBorrowed::new_nfc().is_normalized(note));
    assert!(DecomposingNormalizerBorrowed::new_nfd().is_normalized(note));
    assert!(ComposingNormalizerBorrowed::new_nfkc().is_normalized(note));
    assert!(DecomposingNormalizerBorrowed::new_nfkd().is_normalized(note));

    // -- Precomposed umlaut: NFC/NFKC yes, NFD/NFKD no --
    let umlaut = "a\u{00E4}a"; // "aäa"
    assert!(umlaut.is_nfc(), "'aäa' should be NFC");
    assert!(!umlaut.is_nfd(), "'aäa' should NOT be NFD");
    assert!(umlaut.is_nfkc(), "'aäa' should be NFKC");
    assert!(!umlaut.is_nfkd(), "'aäa' should NOT be NFKD");
    // Cross-validate
    assert!(ComposingNormalizerBorrowed::new_nfc().is_normalized(umlaut));
    assert!(!DecomposingNormalizerBorrowed::new_nfd().is_normalized(umlaut));
    assert!(ComposingNormalizerBorrowed::new_nfkc().is_normalized(umlaut));
    assert!(!DecomposingNormalizerBorrowed::new_nfkd().is_normalized(umlaut));

    // -- Vulgar fraction 1/2 (U+00BD): NFD/NFC yes, NFKD/NFKC no --
    // It has a compatibility decomposition but no canonical one.
    let fraction = "a\u{00BD}a"; // "a½a"
    assert!(fraction.is_nfc(), "'a½a' should be NFC");
    assert!(fraction.is_nfd(), "'a½a' should be NFD");
    assert!(!fraction.is_nfkc(), "'a½a' should NOT be NFKC");
    assert!(!fraction.is_nfkd(), "'a½a' should NOT be NFKD");
    // Cross-validate
    assert!(ComposingNormalizerBorrowed::new_nfc().is_normalized(fraction));
    assert!(DecomposingNormalizerBorrowed::new_nfd().is_normalized(fraction));
    assert!(!ComposingNormalizerBorrowed::new_nfkc().is_normalized(fraction));
    assert!(!DecomposingNormalizerBorrowed::new_nfkd().is_normalized(fraction));
}
