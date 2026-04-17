// tests/composition_exclusions.rs
//
// Tests for Unicode composition exclusions and singleton decompositions.
//
// Composition exclusions are characters that have canonical decompositions but
// are excluded from canonical composition (NFC). This means they decompose in
// NFD but never appear as the *result* of NFC composition. The exclusions fall
// into several categories tested here:
//
//   1. Singleton decompositions (characters mapping to a single character):
//      U+2126 OHM SIGN, U+212A KELVIN SIGN, U+212B ANGSTROM SIGN
//   2. Script-specific singletons:
//      Devanagari U+0958..U+095F, Hebrew presentation forms U+FB1D..U+FB4F
//   3. Combining mark singletons:
//      U+0340 COMBINING GRAVE TONE MARK, U+0341 COMBINING ACUTE TONE MARK,
//      U+0344 COMBINING GREEK DIALYTIKA TONOS
//   4. Verification that these characters cannot be produced by NFC composition

use icu_normalizer::{ComposingNormalizerBorrowed, DecomposingNormalizerBorrowed};
use simd_normalizer::UnicodeNormalization;

// ---------------------------------------------------------------------------
// Helpers (same pattern as tests/special_chars.rs)
// ---------------------------------------------------------------------------

fn assert_nfc(input: &str, expected: &str) {
    let simd = simd_normalizer::nfc().normalize(input);
    let icu = ComposingNormalizerBorrowed::new_nfc().normalize(input);
    assert_eq!(
        &*simd, expected,
        "NFC mismatch for input {:?}: got {:?}, expected {:?}",
        input, simd, expected
    );
    assert_eq!(
        &*simd, &*icu,
        "NFC cross-validation for {:?}: simd={:?}, icu={:?}",
        input, simd, icu
    );
}

fn assert_nfd(input: &str, expected: &str) {
    let simd = simd_normalizer::nfd().normalize(input);
    let icu = DecomposingNormalizerBorrowed::new_nfd().normalize(input);
    assert_eq!(
        &*simd, expected,
        "NFD mismatch for input {:?}: got {:?}, expected {:?}",
        input, simd, expected
    );
    assert_eq!(
        &*simd, &*icu,
        "NFD cross-validation for {:?}: simd={:?}, icu={:?}",
        input, simd, icu
    );
}

fn assert_nfkc(input: &str, expected: &str) {
    let simd = simd_normalizer::nfkc().normalize(input);
    let icu = ComposingNormalizerBorrowed::new_nfkc().normalize(input);
    assert_eq!(
        &*simd, expected,
        "NFKC mismatch for input {:?}: got {:?}, expected {:?}",
        input, simd, expected
    );
    assert_eq!(
        &*simd, &*icu,
        "NFKC cross-validation for {:?}: simd={:?}, icu={:?}",
        input, simd, icu
    );
}

fn assert_nfkd(input: &str, expected: &str) {
    let simd = simd_normalizer::nfkd().normalize(input);
    let icu = DecomposingNormalizerBorrowed::new_nfkd().normalize(input);
    assert_eq!(
        &*simd, expected,
        "NFKD mismatch for input {:?}: got {:?}, expected {:?}",
        input, simd, expected
    );
    assert_eq!(
        &*simd, &*icu,
        "NFKD cross-validation for {:?}: simd={:?}, icu={:?}",
        input, simd, icu
    );
}

fn assert_is_normalized_all(input: &str, nfc: bool, nfd: bool, nfkc: bool, nfkd: bool) {
    assert_eq!(
        input.is_nfc(),
        nfc,
        "is_nfc mismatch for {:?}: expected {}",
        input, nfc
    );
    assert_eq!(
        input.is_nfd(),
        nfd,
        "is_nfd mismatch for {:?}: expected {}",
        input, nfd
    );
    assert_eq!(
        input.is_nfkc(),
        nfkc,
        "is_nfkc mismatch for {:?}: expected {}",
        input, nfkc
    );
    assert_eq!(
        input.is_nfkd(),
        nfkd,
        "is_nfkd mismatch for {:?}: expected {}",
        input, nfkd
    );

    // Cross-validate with ICU
    assert_eq!(
        ComposingNormalizerBorrowed::new_nfc().is_normalized(input),
        nfc,
        "ICU is_nfc mismatch for {:?}",
        input
    );
    assert_eq!(
        DecomposingNormalizerBorrowed::new_nfd().is_normalized(input),
        nfd,
        "ICU is_nfd mismatch for {:?}",
        input
    );
    assert_eq!(
        ComposingNormalizerBorrowed::new_nfkc().is_normalized(input),
        nfkc,
        "ICU is_nfkc mismatch for {:?}",
        input
    );
    assert_eq!(
        DecomposingNormalizerBorrowed::new_nfkd().is_normalized(input),
        nfkd,
        "ICU is_nfkd mismatch for {:?}",
        input
    );
}

// ===========================================================================
// 1. Singleton Exclusions: Symbols that decompose to a single character
// ===========================================================================

// ---- U+2126 OHM SIGN -> U+03A9 GREEK CAPITAL LETTER OMEGA ----

#[test]
fn ohm_sign_decomposes_to_omega_nfc() {
    // U+2126 OHM SIGN is a singleton decomposition to U+03A9 GREEK CAPITAL
    // LETTER OMEGA. In NFC it is replaced by the canonical equivalent.
    assert_nfc("\u{2126}", "\u{03A9}");
}

#[test]
fn ohm_sign_decomposes_to_omega_nfd() {
    assert_nfd("\u{2126}", "\u{03A9}");
}

#[test]
fn ohm_sign_decomposes_to_omega_nfkc() {
    assert_nfkc("\u{2126}", "\u{03A9}");
}

#[test]
fn ohm_sign_decomposes_to_omega_nfkd() {
    assert_nfkd("\u{2126}", "\u{03A9}");
}

#[test]
fn ohm_sign_is_not_normalized() {
    // U+2126 is not in any normalized form (it always maps to U+03A9).
    assert_is_normalized_all("\u{2126}", false, false, false, false);
}

#[test]
fn omega_cannot_compose_back_to_ohm() {
    // U+03A9 is already NFC-stable; it must never produce U+2126.
    let omega = "\u{03A9}";
    assert_nfc(omega, omega);
    assert_nfd(omega, omega);
    assert_is_normalized_all(omega, true, true, true, true);
}

// ---- U+212A KELVIN SIGN -> U+004B LATIN CAPITAL LETTER K ----

#[test]
fn kelvin_sign_decomposes_to_k_nfc() {
    assert_nfc("\u{212A}", "K");
}

#[test]
fn kelvin_sign_decomposes_to_k_nfd() {
    assert_nfd("\u{212A}", "K");
}

#[test]
fn kelvin_sign_decomposes_to_k_nfkc() {
    assert_nfkc("\u{212A}", "K");
}

#[test]
fn kelvin_sign_decomposes_to_k_nfkd() {
    assert_nfkd("\u{212A}", "K");
}

#[test]
fn kelvin_sign_is_not_normalized() {
    assert_is_normalized_all("\u{212A}", false, false, false, false);
}

#[test]
fn ascii_k_cannot_compose_to_kelvin() {
    let k = "K";
    assert_nfc(k, k);
    assert_nfd(k, k);
    assert_is_normalized_all(k, true, true, true, true);
}

// ---- U+212B ANGSTROM SIGN -> U+00C5 LATIN CAPITAL LETTER A WITH RING ABOVE ----
//
// Note: U+212B -> U+00C5 in NFC (singleton exclusion).
// In NFD, U+00C5 further decomposes to U+0041 + U+030A.

#[test]
fn angstrom_decomposes_to_a_ring_nfc() {
    // NFC: U+212B -> U+00C5 (A with ring above, the composed form)
    assert_nfc("\u{212B}", "\u{00C5}");
}

#[test]
fn angstrom_decomposes_to_a_ring_nfd() {
    // NFD: U+212B -> U+0041 U+030A (A + combining ring above)
    assert_nfd("\u{212B}", "A\u{030A}");
}

#[test]
fn angstrom_decomposes_to_a_ring_nfkc() {
    assert_nfkc("\u{212B}", "\u{00C5}");
}

#[test]
fn angstrom_decomposes_to_a_ring_nfkd() {
    assert_nfkd("\u{212B}", "A\u{030A}");
}

#[test]
fn angstrom_is_not_normalized() {
    assert_is_normalized_all("\u{212B}", false, false, false, false);
}

#[test]
fn a_ring_cannot_compose_to_angstrom() {
    // U+00C5 is the canonical composed form; NFC must keep it, not produce U+212B.
    let a_ring = "\u{00C5}";
    assert_nfc(a_ring, a_ring);
    assert_nfd(a_ring, "A\u{030A}");
    assert_is_normalized_all(a_ring, true, false, true, false);
}

// ---- All three singletons in context ----

#[test]
fn singleton_exclusions_in_text() {
    // All three singleton exclusions embedded in text
    let input = "Temp=300\u{212A}, R=5\u{2126}, d=3\u{212B}";
    let expected_nfc = "Temp=300K, R=5\u{03A9}, d=3\u{00C5}";
    let expected_nfd = "Temp=300K, R=5\u{03A9}, d=3A\u{030A}";
    assert_nfc(input, expected_nfc);
    assert_nfd(input, expected_nfd);
    assert_nfkc(input, expected_nfc);
    assert_nfkd(input, expected_nfd);
}

// ===========================================================================
// 2. Combining Mark Singletons
// ===========================================================================

// ---- U+0340 COMBINING GRAVE TONE MARK -> U+0300 COMBINING GRAVE ACCENT ----

#[test]
fn combining_grave_tone_mark_nfc() {
    // U+0340 is a singleton decomposition mapping to U+0300 in NFC/NFD.
    // After a base character, the tone mark should become a grave accent and
    // then compose with the base if possible.
    // 'a' + U+0340 -> NFC: U+00E0 (a with grave, since U+0340 -> U+0300, then a+0300 -> 00E0)
    assert_nfc("a\u{0340}", "\u{00E0}");
}

#[test]
fn combining_grave_tone_mark_nfd() {
    // NFD: a + U+0340 -> a + U+0300 (the singleton is replaced)
    assert_nfd("a\u{0340}", "a\u{0300}");
}

#[test]
fn combining_grave_tone_mark_nfkc() {
    assert_nfkc("a\u{0340}", "\u{00E0}");
}

#[test]
fn combining_grave_tone_mark_nfkd() {
    assert_nfkd("a\u{0340}", "a\u{0300}");
}

#[test]
fn combining_grave_tone_mark_is_not_normalized() {
    // U+0340 alone is not in any normalized form.
    assert_is_normalized_all("\u{0340}", false, false, false, false);
}

#[test]
fn combining_grave_tone_mark_cannot_be_composition_target() {
    // U+0300 COMBINING GRAVE ACCENT is the canonical form; NFC must keep it,
    // never producing U+0340.
    let grave = "a\u{0300}";
    assert_nfc(grave, "\u{00E0}");
    assert_nfd(grave, "a\u{0300}");
    // Verify U+0300 is present in NFD output, not U+0340
    let nfd_result = simd_normalizer::nfd().normalize(grave);
    assert!(
        !nfd_result.contains('\u{0340}'),
        "NFD output must not contain U+0340 COMBINING GRAVE TONE MARK"
    );
}

// ---- U+0341 COMBINING ACUTE TONE MARK -> U+0301 COMBINING ACUTE ACCENT ----

#[test]
fn combining_acute_tone_mark_nfc() {
    // 'e' + U+0341 -> NFC: U+00E9 (e with acute, since U+0341 -> U+0301)
    assert_nfc("e\u{0341}", "\u{00E9}");
}

#[test]
fn combining_acute_tone_mark_nfd() {
    assert_nfd("e\u{0341}", "e\u{0301}");
}

#[test]
fn combining_acute_tone_mark_nfkc() {
    assert_nfkc("e\u{0341}", "\u{00E9}");
}

#[test]
fn combining_acute_tone_mark_nfkd() {
    assert_nfkd("e\u{0341}", "e\u{0301}");
}

#[test]
fn combining_acute_tone_mark_is_not_normalized() {
    assert_is_normalized_all("\u{0341}", false, false, false, false);
}

#[test]
fn combining_acute_tone_mark_cannot_be_composition_target() {
    let acute = "e\u{0301}";
    assert_nfc(acute, "\u{00E9}");
    assert_nfd(acute, "e\u{0301}");
    let nfd_result = simd_normalizer::nfd().normalize(acute);
    assert!(
        !nfd_result.contains('\u{0341}'),
        "NFD output must not contain U+0341 COMBINING ACUTE TONE MARK"
    );
}

// ---- U+0344 COMBINING GREEK DIALYTIKA TONOS -> U+0308 + U+0301 ----

#[test]
fn combining_greek_dialytika_tonos_nfc() {
    // U+0344 decomposes to U+0308 (COMBINING DIAERESIS) + U+0301 (COMBINING ACUTE ACCENT).
    // After 'α' (U+03B1), the two combining marks remain separate in NFC because
    // there is no single precomposed form for alpha + diaeresis + acute.
    // However, for iota: U+03B9 + U+0344 -> U+03B9 + U+0308 + U+0301 -> U+03CA + U+0301 -> U+0390
    assert_nfc("\u{03B9}\u{0344}", "\u{0390}");
}

#[test]
fn combining_greek_dialytika_tonos_nfd() {
    // NFD: U+0344 always decomposes to U+0308 + U+0301
    assert_nfd("\u{03B9}\u{0344}", "\u{03B9}\u{0308}\u{0301}");
}

#[test]
fn combining_greek_dialytika_tonos_nfkc() {
    assert_nfkc("\u{03B9}\u{0344}", "\u{0390}");
}

#[test]
fn combining_greek_dialytika_tonos_nfkd() {
    assert_nfkd("\u{03B9}\u{0344}", "\u{03B9}\u{0308}\u{0301}");
}

#[test]
fn combining_greek_dialytika_tonos_is_not_normalized() {
    assert_is_normalized_all("\u{0344}", false, false, false, false);
}

#[test]
fn combining_greek_dialytika_tonos_on_upsilon() {
    // U+03C5 (upsilon) + U+0344 -> U+03C5 + U+0308 + U+0301 -> U+03CB + U+0301 -> U+03B0
    assert_nfc("\u{03C5}\u{0344}", "\u{03B0}");
    assert_nfd("\u{03C5}\u{0344}", "\u{03C5}\u{0308}\u{0301}");
}

#[test]
fn combining_greek_dialytika_tonos_cannot_be_composition_target() {
    // The sequence U+0308 + U+0301 must never compose back to U+0344.
    let input = "\u{03B9}\u{0308}\u{0301}";
    let nfc_result = simd_normalizer::nfc().normalize(input);
    assert!(
        !nfc_result.contains('\u{0344}'),
        "NFC output must not contain U+0344 COMBINING GREEK DIALYTIKA TONOS"
    );
}

// ---- Combining mark singletons in isolation (no base character) ----

#[test]
fn combining_mark_singletons_in_isolation() {
    // U+0340 alone
    assert_nfc("\u{0340}", "\u{0300}");
    assert_nfd("\u{0340}", "\u{0300}");

    // U+0341 alone
    assert_nfc("\u{0341}", "\u{0301}");
    assert_nfd("\u{0341}", "\u{0301}");

    // U+0344 alone decomposes to two marks
    assert_nfc("\u{0344}", "\u{0308}\u{0301}");
    assert_nfd("\u{0344}", "\u{0308}\u{0301}");
}

// ===========================================================================
// 3. Devanagari Singletons: U+0958..U+095F
// ===========================================================================
//
// Each of these decomposes to a base consonant + U+093C (DEVANAGARI SIGN NUKTA).
// They are composition exclusions: the base+nukta sequence never composes back.
//
//   U+0958 -> U+0915 + U+093C (KA + NUKTA)
//   U+0959 -> U+0916 + U+093C (KHA + NUKTA)
//   U+095A -> U+0917 + U+093C (GA + NUKTA)
//   U+095B -> U+091C + U+093C (JA + NUKTA)
//   U+095C -> U+0921 + U+093C (DDA + NUKTA)
//   U+095D -> U+0922 + U+093C (DDHA + NUKTA)
//   U+095E -> U+092B + U+093C (PHA + NUKTA)
//   U+095F -> U+092F + U+093C (YA + NUKTA)

static DEVANAGARI_EXCLUSIONS: [(char, char); 8] = [
    ('\u{0958}', '\u{0915}'), // QA  = KA + NUKTA
    ('\u{0959}', '\u{0916}'), // KHHA = KHA + NUKTA
    ('\u{095A}', '\u{0917}'), // GHHA = GA + NUKTA
    ('\u{095B}', '\u{091C}'), // ZA  = JA + NUKTA
    ('\u{095C}', '\u{0921}'), // DDDHA = DDA + NUKTA
    ('\u{095D}', '\u{0922}'), // RHA = DDHA + NUKTA
    ('\u{095E}', '\u{092B}'), // FA  = PHA + NUKTA
    ('\u{095F}', '\u{092F}'), // YYA = YA + NUKTA
];

#[test]
fn devanagari_exclusions_nfc() {
    // In NFC, each excluded character decomposes to base + nukta and stays
    // decomposed (composition exclusion: they cannot compose back).
    for &(excluded, base) in &DEVANAGARI_EXCLUSIONS {
        let input = String::from(excluded);
        let expected = format!("{}\u{093C}", base);
        assert_nfc(&input, &expected);
    }
}

#[test]
fn devanagari_exclusions_nfd() {
    for &(excluded, base) in &DEVANAGARI_EXCLUSIONS {
        let input = String::from(excluded);
        let expected = format!("{}\u{093C}", base);
        assert_nfd(&input, &expected);
    }
}

#[test]
fn devanagari_exclusions_nfkc() {
    for &(excluded, base) in &DEVANAGARI_EXCLUSIONS {
        let input = String::from(excluded);
        let expected = format!("{}\u{093C}", base);
        assert_nfkc(&input, &expected);
    }
}

#[test]
fn devanagari_exclusions_nfkd() {
    for &(excluded, base) in &DEVANAGARI_EXCLUSIONS {
        let input = String::from(excluded);
        let expected = format!("{}\u{093C}", base);
        assert_nfkd(&input, &expected);
    }
}

#[test]
fn devanagari_exclusions_is_not_normalized() {
    for &(excluded, _) in &DEVANAGARI_EXCLUSIONS {
        let input = String::from(excluded);
        assert_is_normalized_all(&input, false, false, false, false);
    }
}

#[test]
fn devanagari_base_plus_nukta_cannot_compose_back() {
    // Verify that base + nukta stays as-is in NFC (composition exclusion).
    for &(excluded, base) in &DEVANAGARI_EXCLUSIONS {
        let input = format!("{}\u{093C}", base);
        // NFC must keep base + nukta; it must NOT compose back to the excluded char.
        let nfc_result = simd_normalizer::nfc().normalize(&input);
        assert!(
            !nfc_result.contains(excluded),
            "NFC of base {:04X} + nukta must not produce excluded char {:04X}, got {:?}",
            base as u32,
            excluded as u32,
            nfc_result
        );
        // Also verify the output equals the input (base + nukta is already NFC)
        assert_nfc(&input, &input);
    }
}

#[test]
fn devanagari_exclusions_in_word_context() {
    // A Devanagari word using excluded characters, verifying normalization
    // handles them correctly in a sequence.
    // "फ़ारसी" with U+095E (FA) instead of U+092B + U+093C
    let input = "\u{095E}\u{093E}\u{0930}\u{0938}\u{0940}";
    // NFC: U+095E -> U+092B + U+093C, rest stays
    let expected = "\u{092B}\u{093C}\u{093E}\u{0930}\u{0938}\u{0940}";
    assert_nfc(input, expected);
    assert_nfd(input, expected);
}

// ===========================================================================
// 4. Hebrew Presentation Forms (U+FB1D..U+FB4F)
// ===========================================================================
//
// Some of these have canonical decompositions (and are thus composition
// exclusions), while others only have compatibility decompositions.

#[test]
fn hebrew_fb1d_yod_with_hiriq() {
    // U+FB1D HEBREW LETTER YOD WITH HIRIQ -> U+05D9 + U+05B4
    // This is a canonical decomposition and a composition exclusion.
    let input = "\u{FB1D}";
    let expected = "\u{05D9}\u{05B4}";
    assert_nfc(input, expected);
    assert_nfd(input, expected);
    assert_nfkc(input, expected);
    assert_nfkd(input, expected);
    assert_is_normalized_all(input, false, false, false, false);
}

#[test]
fn hebrew_fb1d_cannot_compose_back() {
    // U+05D9 + U+05B4 must not compose to U+FB1D in NFC.
    let input = "\u{05D9}\u{05B4}";
    let nfc_result = simd_normalizer::nfc().normalize(input);
    assert!(
        !nfc_result.contains('\u{FB1D}'),
        "NFC of yod + hiriq must not produce U+FB1D, got {:?}",
        nfc_result
    );
    assert_nfc(input, input);
}

#[test]
fn hebrew_fb2a_shin_with_shin_dot() {
    // U+FB2A HEBREW LETTER SHIN WITH SHIN DOT -> U+05E9 + U+05C1
    // Canonical decomposition, composition exclusion.
    let input = "\u{FB2A}";
    let expected = "\u{05E9}\u{05C1}";
    assert_nfc(input, expected);
    assert_nfd(input, expected);
    assert_nfkc(input, expected);
    assert_nfkd(input, expected);
    assert_is_normalized_all(input, false, false, false, false);
}

#[test]
fn hebrew_fb2b_shin_with_sin_dot() {
    // U+FB2B HEBREW LETTER SHIN WITH SIN DOT -> U+05E9 + U+05C2
    let input = "\u{FB2B}";
    let expected = "\u{05E9}\u{05C2}";
    assert_nfc(input, expected);
    assert_nfd(input, expected);
    assert_nfkc(input, expected);
    assert_nfkd(input, expected);
}

#[test]
fn hebrew_fb2a_shin_dot_cannot_compose_back() {
    let input = "\u{05E9}\u{05C1}";
    let nfc_result = simd_normalizer::nfc().normalize(input);
    assert!(
        !nfc_result.contains('\u{FB2A}'),
        "NFC must not produce U+FB2A"
    );
    assert_nfc(input, input);
}

#[test]
fn hebrew_fb49_shin_with_dagesh() {
    // U+FB49 HEBREW LETTER SHIN WITH DAGESH -> U+05E9 + U+05BC
    // Canonical decomposition, composition exclusion.
    let input = "\u{FB49}";
    let expected = "\u{05E9}\u{05BC}";
    assert_nfc(input, expected);
    assert_nfd(input, expected);
    assert_nfkc(input, expected);
    assert_nfkd(input, expected);
}

#[test]
fn hebrew_compatibility_forms_nfkc() {
    // U+FB20 HEBREW LETTER ALTERNATIVE AYIN -> U+05E2 (compatibility mapping only)
    // This should only decompose in NFKC/NFKD, not NFC/NFD.
    let input = "\u{FB20}";
    // NFC/NFD: unchanged (no canonical decomposition)
    assert_nfc(input, input);
    assert_nfd(input, input);
    // NFKC/NFKD: decomposes to base letter
    assert_nfkc(input, "\u{05E2}");
    assert_nfkd(input, "\u{05E2}");
}

#[test]
fn hebrew_fb4f_ligature_alef_lamed() {
    // U+FB4F HEBREW LIGATURE ALEF LAMED -> U+05D0 + U+05DC (compatibility only)
    let input = "\u{FB4F}";
    assert_nfc(input, input);
    assert_nfd(input, input);
    assert_nfkc(input, "\u{05D0}\u{05DC}");
    assert_nfkd(input, "\u{05D0}\u{05DC}");
}

// ===========================================================================
// 5. Composition exclusion verification: roundtrip stability
// ===========================================================================

#[test]
fn nfc_idempotent_for_all_singletons() {
    // For every composition exclusion tested, NFC of NFC must equal NFC.
    let exclusions: Vec<&str> = vec![
        "\u{2126}",
        "\u{212A}",
        "\u{212B}",
        "\u{0340}",
        "\u{0341}",
        "\u{0344}",
        "\u{0958}",
        "\u{0959}",
        "\u{095A}",
        "\u{095B}",
        "\u{095C}",
        "\u{095D}",
        "\u{095E}",
        "\u{095F}",
        "\u{FB1D}",
        "\u{FB2A}",
        "\u{FB2B}",
        "\u{FB49}",
    ];

    for input in &exclusions {
        let nfc_once = simd_normalizer::nfc().normalize(input);
        let nfc_twice = simd_normalizer::nfc().normalize(&nfc_once);
        assert_eq!(
            &*nfc_once, &*nfc_twice,
            "NFC is not idempotent for {:?}: first={:?}, second={:?}",
            input, nfc_once, nfc_twice
        );
    }
}

#[test]
fn nfd_idempotent_for_all_singletons() {
    let exclusions: Vec<&str> = vec![
        "\u{2126}",
        "\u{212A}",
        "\u{212B}",
        "\u{0340}",
        "\u{0341}",
        "\u{0344}",
        "\u{0958}",
        "\u{0959}",
        "\u{095A}",
        "\u{095B}",
        "\u{095C}",
        "\u{095D}",
        "\u{095E}",
        "\u{095F}",
        "\u{FB1D}",
        "\u{FB2A}",
        "\u{FB2B}",
        "\u{FB49}",
    ];

    for input in &exclusions {
        let nfd_once = simd_normalizer::nfd().normalize(input);
        let nfd_twice = simd_normalizer::nfd().normalize(&nfd_once);
        assert_eq!(
            &*nfd_once, &*nfd_twice,
            "NFD is not idempotent for {:?}: first={:?}, second={:?}",
            input, nfd_once, nfd_twice
        );
    }
}

#[test]
fn excluded_chars_never_appear_in_nfc_output() {
    // The definitive composition exclusion property: these characters must
    // NEVER appear in NFC-normalized text, regardless of input.
    let excluded_chars: Vec<char> = vec![
        '\u{2126}', '\u{212A}', '\u{212B}', // Symbol singletons
        '\u{0340}', '\u{0341}', '\u{0344}', // Combining mark singletons
        '\u{0958}', '\u{0959}', '\u{095A}', '\u{095B}', // Devanagari
        '\u{095C}', '\u{095D}', '\u{095E}', '\u{095F}',
        '\u{FB1D}', '\u{FB2A}', '\u{FB2B}', '\u{FB49}', // Hebrew
    ];

    for &ch in &excluded_chars {
        let input = String::from(ch);
        let nfc_result = simd_normalizer::nfc().normalize(&input);
        assert!(
            !nfc_result.contains(ch),
            "NFC output for U+{:04X} must not contain the excluded character itself, got {:?}",
            ch as u32,
            nfc_result
        );
    }
}

// ===========================================================================
// 6. Combining tone marks in realistic sequences
// ===========================================================================

#[test]
fn combining_grave_tone_mark_with_various_bases() {
    // Vietnamese-style: 'o' + U+0340 -> NFC should produce o-grave (U+00F2)
    assert_nfc("o\u{0340}", "\u{00F2}");
    assert_nfd("o\u{0340}", "o\u{0300}");

    // 'E' + U+0340 -> E-grave (U+00C8)
    assert_nfc("E\u{0340}", "\u{00C8}");
    assert_nfd("E\u{0340}", "E\u{0300}");

    // 'u' + U+0340 -> u-grave (U+00F9)
    assert_nfc("u\u{0340}", "\u{00F9}");
}

#[test]
fn combining_acute_tone_mark_with_various_bases() {
    // 'a' + U+0341 -> a-acute (U+00E1)
    assert_nfc("a\u{0341}", "\u{00E1}");
    assert_nfd("a\u{0341}", "a\u{0301}");

    // 'O' + U+0341 -> O-acute (U+00D3)
    assert_nfc("O\u{0341}", "\u{00D3}");
    assert_nfd("O\u{0341}", "O\u{0301}");
}

#[test]
fn tone_marks_stacked_with_other_combining() {
    // a + U+0340 (-> U+0300) + U+0302 (circumflex)
    // CCC of U+0300 = 230, CCC of U+0302 = 230 (equal, so blocked from composing
    // circumflex with base). NFC: a + U+0300 compose -> U+00E0, then U+0302 stays.
    assert_nfc("a\u{0340}\u{0302}", "\u{00E0}\u{0302}");
    assert_nfd("a\u{0340}\u{0302}", "a\u{0300}\u{0302}");
}

#[test]
fn dialytika_tonos_in_greek_text() {
    // Greek text with U+0344: "ΐ" can be written as U+03B9 + U+0344
    // NFC -> U+0390 (GREEK SMALL LETTER IOTA WITH DIALYTIKA AND TONOS)
    let input = "\u{03B9}\u{0344}";
    assert_nfc(input, "\u{0390}");
    assert_nfd(input, "\u{03B9}\u{0308}\u{0301}");

    // Full word context: "ελληνικά"
    let word_with_dialytika = "\u{03B5}\u{03BB}\u{03BB}\u{03B7}\u{03BD}\u{03B9}\u{0344}";
    let nfc_result = simd_normalizer::nfc().normalize(word_with_dialytika);
    let icu_result = ComposingNormalizerBorrowed::new_nfc().normalize(word_with_dialytika);
    assert_eq!(&*nfc_result, &*icu_result, "Greek word cross-validation");
    assert!(
        !nfc_result.contains('\u{0344}'),
        "NFC Greek text must not contain U+0344"
    );
}

// ===========================================================================
// 7. Edge cases and cross-form consistency
// ===========================================================================

#[test]
fn nfc_then_nfd_roundtrip() {
    // NFC -> NFD -> NFC must be stable for composition exclusions.
    let inputs = [
        "\u{2126}", "\u{212A}", "\u{212B}",
        "\u{0340}", "\u{0341}", "\u{0344}",
        "\u{0958}", "\u{095F}",
        "\u{FB1D}", "\u{FB2A}",
    ];

    for input in &inputs {
        let nfc1 = simd_normalizer::nfc().normalize(input);
        let nfd1 = simd_normalizer::nfd().normalize(&nfc1);
        let nfc2 = simd_normalizer::nfc().normalize(&nfd1);
        assert_eq!(
            &*nfc1, &*nfc2,
            "NFC->NFD->NFC roundtrip unstable for {:?}",
            input
        );
    }
}

#[test]
fn nfkc_agrees_with_nfc_for_canonical_exclusions() {
    // For characters with canonical (not compatibility) decompositions,
    // NFC and NFKC should produce the same result.
    let canonical_exclusions = [
        "\u{2126}", "\u{212A}", "\u{212B}",
        "\u{0340}", "\u{0341}", "\u{0344}",
        "\u{0958}", "\u{095F}",
        "\u{FB1D}", "\u{FB2A}",
    ];

    for input in &canonical_exclusions {
        let nfc_result = simd_normalizer::nfc().normalize(input);
        let nfkc_result = simd_normalizer::nfkc().normalize(input);
        assert_eq!(
            &*nfc_result, &*nfkc_result,
            "NFC and NFKC disagree for canonical exclusion {:?}: nfc={:?}, nfkc={:?}",
            input, nfc_result, nfkc_result
        );
    }
}

#[test]
fn nfkd_agrees_with_nfd_for_canonical_exclusions() {
    let canonical_exclusions = [
        "\u{2126}", "\u{212A}", "\u{212B}",
        "\u{0340}", "\u{0341}", "\u{0344}",
        "\u{0958}", "\u{095F}",
        "\u{FB1D}", "\u{FB2A}",
    ];

    for input in &canonical_exclusions {
        let nfd_result = simd_normalizer::nfd().normalize(input);
        let nfkd_result = simd_normalizer::nfkd().normalize(input);
        assert_eq!(
            &*nfd_result, &*nfkd_result,
            "NFD and NFKD disagree for canonical exclusion {:?}: nfd={:?}, nfkd={:?}",
            input, nfd_result, nfkd_result
        );
    }
}

#[test]
fn multiple_exclusions_in_single_string() {
    // A string containing multiple composition exclusions from different scripts
    let input = "\u{2126}\u{212A}\u{212B}\u{0958}\u{FB1D}";
    let nfc_result = simd_normalizer::nfc().normalize(input);
    let icu_result = ComposingNormalizerBorrowed::new_nfc().normalize(input);
    assert_eq!(
        &*nfc_result, &*icu_result,
        "Multi-script exclusion cross-validation"
    );
    // None of the excluded characters should appear in the output
    for ch in ['\u{2126}', '\u{212A}', '\u{212B}', '\u{0958}', '\u{FB1D}'] {
        assert!(
            !nfc_result.contains(ch),
            "NFC output should not contain U+{:04X}",
            ch as u32
        );
    }
}
