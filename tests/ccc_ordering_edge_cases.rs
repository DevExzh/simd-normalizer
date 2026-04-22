// tests/ccc_ordering_edge_cases.rs
//
// Edge case tests focused on Canonical Combining Class (CCC) ordering.
//
// The CCC sort must be *stable*: combining marks with the same CCC value
// must preserve their relative order from the input. The library uses an
// inline buffer (CccBuffer) for up to 18 entries and overflows to Vec for
// longer sequences; both paths must produce identical, correct results.
//
// Every test cross-validates against ICU4X (icu_normalizer) as the
// reference implementation.

use icu_normalizer::{ComposingNormalizerBorrowed, DecomposingNormalizerBorrowed};

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
// simd-normalizer helpers
// ---------------------------------------------------------------------------

fn our_nfc(s: &str) -> String {
    simd_normalizer::nfc().normalize(s).into_owned()
}

fn our_nfd(s: &str) -> String {
    simd_normalizer::nfd().normalize(s).into_owned()
}

fn our_nfkc(s: &str) -> String {
    simd_normalizer::nfkc().normalize(s).into_owned()
}

fn our_nfkd(s: &str) -> String {
    simd_normalizer::nfkd().normalize(s).into_owned()
}

// ---------------------------------------------------------------------------
// Formatting helper for diagnostics
// ---------------------------------------------------------------------------

fn codepoints(s: &str) -> String {
    s.chars()
        .map(|c| format!("U+{:04X}", c as u32))
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------------------------------------------------------------------------
// Assertion helpers: compare simd-normalizer output against ICU4X for all forms
// ---------------------------------------------------------------------------

fn assert_nfd_matches_icu(label: &str, input: &str) {
    let ours = our_nfd(input);
    let reference = icu_nfd(input);
    assert_eq!(
        ours,
        reference,
        "NFD mismatch [{label}]\n  input: {}\n  ours:  {}\n  icu:   {}",
        codepoints(input),
        codepoints(&ours),
        codepoints(&reference),
    );
}

fn assert_nfc_matches_icu(label: &str, input: &str) {
    let ours = our_nfc(input);
    let reference = icu_nfc(input);
    assert_eq!(
        ours,
        reference,
        "NFC mismatch [{label}]\n  input: {}\n  ours:  {}\n  icu:   {}",
        codepoints(input),
        codepoints(&ours),
        codepoints(&reference),
    );
}

fn assert_nfkd_matches_icu(label: &str, input: &str) {
    let ours = our_nfkd(input);
    let reference = icu_nfkd(input);
    assert_eq!(
        ours,
        reference,
        "NFKD mismatch [{label}]\n  input: {}\n  ours:  {}\n  icu:   {}",
        codepoints(input),
        codepoints(&ours),
        codepoints(&reference),
    );
}

fn assert_nfkc_matches_icu(label: &str, input: &str) {
    let ours = our_nfkc(input);
    let reference = icu_nfkc(input);
    assert_eq!(
        ours,
        reference,
        "NFKC mismatch [{label}]\n  input: {}\n  ours:  {}\n  icu:   {}",
        codepoints(input),
        codepoints(&ours),
        codepoints(&reference),
    );
}

/// Assert all four normalization forms match ICU4X.
fn assert_all_forms_match_icu(label: &str, input: &str) {
    assert_nfd_matches_icu(label, input);
    assert_nfc_matches_icu(label, input);
    assert_nfkd_matches_icu(label, input);
    assert_nfkc_matches_icu(label, input);
}

// ===========================================================================
// 1. Stable sort verification: multiple marks with the same CCC value
// ===========================================================================

#[test]
fn stable_sort_same_ccc_above_marks() {
    // Multiple CCC 230 (above) marks on one base character.
    // The canonical ordering sort must be stable: marks with the same CCC
    // value must preserve their original relative order.
    //
    // U+0300 COMBINING GRAVE ACCENT (CCC 230)
    // U+0301 COMBINING ACUTE ACCENT (CCC 230)
    // U+0302 COMBINING CIRCUMFLEX ACCENT (CCC 230)
    // U+0303 COMBINING TILDE (CCC 230)
    // U+0304 COMBINING MACRON (CCC 230)
    // U+0308 COMBINING DIAERESIS (CCC 230)
    let input = "a\u{0300}\u{0301}\u{0302}\u{0303}\u{0304}\u{0308}";
    assert_all_forms_match_icu("stable-sort-6-above-marks", input);

    // Reverse order of the same marks (still all CCC 230, so stable sort
    // must keep them in this reversed order after sorting).
    let input_rev = "a\u{0308}\u{0304}\u{0303}\u{0302}\u{0301}\u{0300}";
    assert_all_forms_match_icu("stable-sort-6-above-marks-reversed", input_rev);
}

#[test]
fn stable_sort_same_ccc_below_marks() {
    // Multiple CCC 220 (below) marks on one base.
    // U+0316 COMBINING GRAVE ACCENT BELOW (CCC 220)
    // U+0317 COMBINING ACUTE ACCENT BELOW (CCC 220)
    // U+0323 COMBINING DOT BELOW (CCC 220)
    // U+0324 COMBINING DIAERESIS BELOW (CCC 220)
    // U+0325 COMBINING RING BELOW (CCC 220)
    let input = "a\u{0316}\u{0317}\u{0323}\u{0324}\u{0325}";
    assert_all_forms_match_icu("stable-sort-5-below-marks", input);
}

#[test]
fn stable_sort_mixed_same_ccc_groups() {
    // Two groups of same-CCC marks interleaved with a different CCC.
    // CCC 230: U+0300, U+0301
    // CCC 220: U+0323, U+0325
    // CCC 230: U+0302, U+0303
    //
    // After sorting by CCC, all CCC 220 marks should come first, then all
    // CCC 230 marks. Within each group, the original relative order must
    // be preserved (stable sort).
    let input = "e\u{0300}\u{0301}\u{0323}\u{0325}\u{0302}\u{0303}";
    assert_all_forms_match_icu("stable-sort-mixed-groups", input);
}

#[test]
fn stable_sort_three_marks_same_ccc_with_composition() {
    // NFC composition with stable ordering.
    // e + U+0323 (CCC 220, dot below) + U+0302 (CCC 230, circumflex) + U+0301 (CCC 230, acute)
    // CCC sort: 220 stays first, then 230s in original order (circumflex then acute).
    // NFC should compose e + U+0323 + U+0302 into U+1EC7, with U+0301 trailing.
    let input = "e\u{0323}\u{0302}\u{0301}";
    assert_all_forms_match_icu("stable-sort-compose-with-trailing", input);
}

// ===========================================================================
// 2. Hebrew combining marks (CCC 10-26 range)
// ===========================================================================

#[test]
fn hebrew_vowel_marks_single_base() {
    // Hebrew letter Bet (U+05D1) with multiple Hebrew vowel points stacked.
    // U+05B0 SHEVA (CCC 10)
    // U+05B1 HATAF SEGOL (CCC 11)
    // U+05B4 HIRIQ (CCC 14)
    // U+05B7 PATAH (CCC 17)
    // U+05B8 QAMATS (CCC 18)
    // U+05BB QUBUTS (CCC 20)
    // U+05BC DAGESH (CCC 21)
    // U+05BD METEG (CCC 22)
    // U+05BF RAFE (CCC 23)
    // U+05C1 SHIN DOT (CCC 24)
    // U+05C2 SIN DOT (CCC 25)

    // Forward order (already sorted by CCC)
    let input_sorted = "\u{05D1}\u{05B0}\u{05B1}\u{05B4}\u{05B7}\u{05B8}\u{05BB}\u{05BC}\u{05BD}\u{05BF}\u{05C1}\u{05C2}";
    assert_all_forms_match_icu("hebrew-all-vowels-sorted", input_sorted);

    // Reverse order (worst-case for sorting)
    let input_reversed = "\u{05D1}\u{05C2}\u{05C1}\u{05BF}\u{05BD}\u{05BC}\u{05BB}\u{05B8}\u{05B7}\u{05B4}\u{05B1}\u{05B0}";
    assert_all_forms_match_icu("hebrew-all-vowels-reversed", input_reversed);
}

#[test]
fn hebrew_dagesh_and_vowel_ordering() {
    // Common Hebrew pattern: letter + dagesh (CCC 21) + vowel (various CCC).
    // The dagesh should sort after vowels with lower CCC.
    //
    // Bet + Dagesh (CCC 21) + Sheva (CCC 10)
    // After canonical ordering: Sheva (10) before Dagesh (21).
    let input = "\u{05D1}\u{05BC}\u{05B0}";
    assert_all_forms_match_icu("hebrew-dagesh-before-sheva", input);

    // Bet + Hiriq (CCC 14) + Dagesh (CCC 21) + Meteg (CCC 22)
    // Already in order; should be stable.
    let input2 = "\u{05D1}\u{05B4}\u{05BC}\u{05BD}";
    assert_all_forms_match_icu("hebrew-hiriq-dagesh-meteg", input2);
}

#[test]
fn hebrew_shin_sin_dot_distinction() {
    // Shin (U+05E9) + Shin Dot (U+05C1, CCC 24) vs Sin Dot (U+05C2, CCC 25)
    // These differ in CCC and should maintain their ordering.
    let shin_with_shin_dot = "\u{05E9}\u{05C1}";
    let shin_with_sin_dot = "\u{05E9}\u{05C2}";

    assert_all_forms_match_icu("hebrew-shin-dot", shin_with_shin_dot);
    assert_all_forms_match_icu("hebrew-sin-dot", shin_with_sin_dot);

    // Both dots plus a vowel: Shin + Sin Dot (CCC 25) + Shin Dot (CCC 24) + Hiriq (CCC 14)
    // After sorting: Hiriq (14) then Shin Dot (24) then Sin Dot (25)
    let input = "\u{05E9}\u{05C2}\u{05C1}\u{05B4}";
    assert_all_forms_match_icu("hebrew-two-dots-plus-vowel", input);
}

#[test]
fn hebrew_realistic_word() {
    // A realistic Hebrew word with multiple marks per letter.
    // Bet + Dagesh + Sheva + Resh + Alef + Shin + Shin Dot + Hiriq + Tav
    let word = "\u{05D1}\u{05BC}\u{05B0}\u{05E8}\u{05D0}\u{05E9}\u{05C1}\u{05B4}\u{05EA}";
    assert_all_forms_match_icu("hebrew-realistic-word", word);
}

// ===========================================================================
// 3. Arabic combining marks (CCC 27-35 range)
// ===========================================================================

#[test]
fn arabic_vowel_marks_single_base() {
    // Arabic letter Ba (U+0628) with various Arabic combining marks.
    // U+064B FATHATAN (CCC 27)
    // U+064C DAMMATAN (CCC 28)
    // U+064D KASRATAN (CCC 29)
    // U+064E FATHA (CCC 30)
    // U+064F DAMMA (CCC 31)
    // U+0650 KASRA (CCC 32)
    // U+0651 SHADDA (CCC 33)
    // U+0652 SUKUN (CCC 34)
    // U+0670 SUPERSCRIPT ALEF (CCC 35)

    // Forward order
    let input_sorted =
        "\u{0628}\u{064B}\u{064C}\u{064D}\u{064E}\u{064F}\u{0650}\u{0651}\u{0652}\u{0670}";
    assert_all_forms_match_icu("arabic-all-marks-sorted", input_sorted);

    // Reverse order
    let input_reversed =
        "\u{0628}\u{0670}\u{0652}\u{0651}\u{0650}\u{064F}\u{064E}\u{064D}\u{064C}\u{064B}";
    assert_all_forms_match_icu("arabic-all-marks-reversed", input_reversed);
}

#[test]
fn arabic_shadda_with_vowel() {
    // Very common Arabic pattern: letter + Shadda (CCC 33) + Fatha (CCC 30)
    // After canonical ordering: Fatha (30) before Shadda (33).
    let input = "\u{0628}\u{0651}\u{064E}";
    assert_all_forms_match_icu("arabic-shadda-fatha", input);

    // Letter + Kasra (CCC 32) + Shadda (CCC 33) -- already sorted
    let input2 = "\u{0628}\u{0650}\u{0651}";
    assert_all_forms_match_icu("arabic-kasra-shadda", input2);

    // Letter + Shadda (33) + Kasra (32) -- needs reorder
    let input3 = "\u{0628}\u{0651}\u{0650}";
    assert_all_forms_match_icu("arabic-shadda-kasra-reorder", input3);
}

#[test]
fn arabic_small_marks() {
    // Arabic small marks with lower CCC values.
    // U+0618 SMALL FATHA (CCC 30)
    // U+0619 SMALL DAMMA (CCC 31)
    // U+061A SMALL KASRA (CCC 32)
    let input = "\u{0628}\u{061A}\u{0619}\u{0618}";
    assert_all_forms_match_icu("arabic-small-marks-reversed", input);
}

#[test]
fn arabic_realistic_word() {
    // Bismillah: Ba + Kasra + Sin + Mim + Alef + Lam + Lam + Ha + ...
    // A realistic Arabic text fragment with diacritics.
    let word = "\u{0628}\u{0650}\u{0633}\u{0652}\u{0645}\u{0650}\u{0627}\u{0644}\u{0644}\u{0651}\u{064E}\u{0647}\u{0650}";
    assert_all_forms_match_icu("arabic-bismillah-fragment", word);
}

// ===========================================================================
// 4. Tibetan combining marks (unusual CCC values)
// ===========================================================================

#[test]
fn tibetan_vowel_signs() {
    // Tibetan base consonant Ka (U+0F40) with various Tibetan combining marks.
    // U+0F71 TIBETAN VOWEL SIGN AA (CCC 129)
    // U+0F72 TIBETAN VOWEL SIGN I (CCC 130)
    // U+0F74 TIBETAN VOWEL SIGN U (CCC 132)
    // U+0F7A TIBETAN VOWEL SIGN E (CCC 130)
    // U+0F7C TIBETAN VOWEL SIGN O (CCC 130)
    // U+0F80 TIBETAN VOWEL SIGN REVERSED I (CCC 130)

    // Mix of CCC 129, 130, 132 marks
    let input = "\u{0F40}\u{0F74}\u{0F72}\u{0F71}";
    assert_all_forms_match_icu("tibetan-vowels-unsorted", input);

    // CCC 130 marks only (stability test)
    let input_same_ccc = "\u{0F40}\u{0F72}\u{0F7A}\u{0F7C}\u{0F80}";
    assert_all_forms_match_icu("tibetan-same-ccc-130", input_same_ccc);
}

#[test]
fn tibetan_subjoined_consonants() {
    // Tibetan subjoined consonants have CCC 0 (they are starters), but
    // Tibetan vowel signs around them stress the sorting.
    // Ka (U+0F40) + subjoined Sa (U+0FB6, CCC 0) + vowel sign I (U+0F72, CCC 130)
    let input = "\u{0F40}\u{0FB6}\u{0F72}";
    assert_all_forms_match_icu("tibetan-subjoined-with-vowel", input);
}

#[test]
fn tibetan_marks_with_latin_marks() {
    // Mix Tibetan CCC values (129, 130, 132) with Latin CCC values (220, 230)
    // on a Latin base to exercise unusual CCC value combinations.
    // 'a' + U+0F39 (CCC 216) + U+0302 (CCC 230) + U+0323 (CCC 220) + U+0F71 (CCC 129)
    // After sorting: CCC 129, 216, 220, 230
    let input = "a\u{0F39}\u{0302}\u{0323}\u{0F71}";
    assert_all_forms_match_icu("tibetan-latin-mixed-ccc", input);
}

// ===========================================================================
// 5. Vietnamese tone marks in combination with other marks
// ===========================================================================

#[test]
fn vietnamese_tone_marks_basic() {
    // Vietnamese uses Latin base + horn/breve + tone mark.
    // U+031B COMBINING HORN (CCC 216)
    // U+0300 COMBINING GRAVE (CCC 230)
    // U+0301 COMBINING ACUTE (CCC 230)
    // U+0303 COMBINING TILDE (CCC 230)
    // U+0309 COMBINING HOOK ABOVE (CCC 230)
    // U+0323 COMBINING DOT BELOW (CCC 220)
    //
    // Common patterns: o + horn + tone mark

    // o + horn (CCC 216) + acute (CCC 230)
    let input1 = "o\u{031B}\u{0301}";
    assert_all_forms_match_icu("vietnamese-o-horn-acute", input1);

    // o + horn (CCC 216) + dot below (CCC 220) + grave (CCC 230)
    let input2 = "o\u{031B}\u{0323}\u{0300}";
    assert_all_forms_match_icu("vietnamese-o-horn-dot-grave", input2);

    // Reversed: o + grave (CCC 230) + dot below (CCC 220) + horn (CCC 216)
    // After sort: horn (216), dot below (220), grave (230)
    let input3 = "o\u{0300}\u{0323}\u{031B}";
    assert_all_forms_match_icu("vietnamese-reversed-marks", input3);
}

#[test]
fn vietnamese_precomposed_base_with_extra_marks() {
    // U+01A1 (o with horn, precomposed) = o + U+031B in NFD.
    // Adding more marks tests composition after decomposition.
    // NFC of (o-horn + acute) should produce U+1EDB.
    let input1 = "\u{01A1}\u{0301}";
    assert_all_forms_match_icu("vietnamese-precomposed-o-horn-acute", input1);

    // U+01A1 + dot below + hook above
    let input2 = "\u{01A1}\u{0323}\u{0309}";
    assert_all_forms_match_icu("vietnamese-precomposed-o-horn-dot-hook", input2);
}

#[test]
fn vietnamese_realistic_word() {
    // "Viet Nam" with full diacritics:
    // V + i + e + circumflex (U+0302) + dot below (U+0323) + t + N + a + breve (U+0306) + m
    // This decomposes/recomposes differently in NFC vs NFD.
    let word = "Vi\u{1EC7}t Nam";
    assert_all_forms_match_icu("vietnamese-viet-nam-precomposed", word);

    // Same word, fully decomposed input
    let word_decomposed = "Vie\u{0323}\u{0302}t Nam";
    assert_all_forms_match_icu("vietnamese-viet-nam-decomposed", word_decomposed);
}

// ===========================================================================
// 6. Very long sequences (50+ marks) to stress CccBuffer overflow to Vec
// ===========================================================================

#[test]
fn long_sequence_50_above_marks() {
    // 50 CCC 230 marks (all COMBINING GRAVE through COMBINING DOUBLE ACUTE)
    // on one base. This exceeds INLINE_CAP=18 and forces Vec overflow.
    let above_marks: &[char] = &[
        '\u{0300}', '\u{0301}', '\u{0302}', '\u{0303}', '\u{0304}', '\u{0305}', '\u{0306}',
        '\u{0307}', '\u{0308}', '\u{030B}',
    ];
    let mut input = String::from("a");
    for i in 0..50 {
        input.push(above_marks[i % above_marks.len()]);
    }
    assert_all_forms_match_icu("long-50-above-marks", &input);
}

#[test]
fn long_sequence_60_mixed_ccc() {
    // 60 marks with varying CCC values, in reverse order.
    // This tests both the overflow path and correct sorting of many elements.
    let marks_and_ccc: &[(char, u8)] = &[
        ('\u{0327}', 202), // cedilla
        ('\u{0328}', 202), // ogonek
        ('\u{0316}', 220), // grave below
        ('\u{0323}', 220), // dot below
        ('\u{0317}', 220), // acute below
        ('\u{0300}', 230), // grave
        ('\u{0301}', 230), // acute
        ('\u{0302}', 230), // circumflex
        ('\u{0308}', 230), // diaeresis
        ('\u{0303}', 230), // tilde
    ];

    // Build in reverse CCC order (worst case for sorting)
    let mut input = String::from("e");
    for i in (0..60).rev() {
        let (ch, _) = marks_and_ccc[i % marks_and_ccc.len()];
        input.push(ch);
    }
    assert_all_forms_match_icu("long-60-mixed-reverse-ccc", &input);
}

#[test]
fn long_sequence_100_marks_all_same_ccc() {
    // 100 marks all with CCC 230 to test stable sort on the Vec overflow path.
    // We cycle through distinct characters to verify order is preserved.
    let marks: &[char] = &[
        '\u{0300}', '\u{0301}', '\u{0302}', '\u{0303}', '\u{0304}', '\u{0305}', '\u{0306}',
        '\u{0307}', '\u{0308}', '\u{030B}',
    ];
    let mut input = String::from("x");
    for i in 0..100 {
        input.push(marks[i % marks.len()]);
    }
    assert_all_forms_match_icu("long-100-same-ccc-230", &input);
}

#[test]
fn long_sequence_boundary_at_inline_cap() {
    // Exactly 18 marks (INLINE_CAP boundary) -- should stay inline.
    let mut input_18 = String::from("a");
    for i in 0..18 {
        // Alternate between CCC 220 and CCC 230
        if i % 2 == 0 {
            input_18.push('\u{0323}'); // dot below (220)
        } else {
            input_18.push('\u{0301}'); // acute (230)
        }
    }
    assert_all_forms_match_icu("long-exactly-18-marks", &input_18);

    // 19 marks (just past INLINE_CAP) -- triggers overflow
    let mut input_19 = input_18.clone();
    input_19.push('\u{0300}'); // grave (230)
    assert_all_forms_match_icu("long-exactly-19-marks-overflow", &input_19);
}

#[test]
fn long_sequence_multiple_ccc_bands() {
    // 54 marks spanning many different CCC values, 3 of each.
    // This tests sorting correctness across a wide CCC range.
    let marks_by_ccc: &[(char, u8)] = &[
        ('\u{0334}', 1),   // tilde overlay (CCC 1)
        ('\u{093C}', 7),   // Devanagari nukta (CCC 7)
        ('\u{094D}', 9),   // Devanagari virama (CCC 9)
        ('\u{05B0}', 10),  // Hebrew sheva (CCC 10)
        ('\u{05B4}', 14),  // Hebrew hiriq (CCC 14)
        ('\u{05BC}', 21),  // Hebrew dagesh (CCC 21)
        ('\u{064B}', 27),  // Arabic fathatan (CCC 27)
        ('\u{064E}', 30),  // Arabic fatha (CCC 30)
        ('\u{0651}', 33),  // Arabic shadda (CCC 33)
        ('\u{0670}', 35),  // Arabic superscript alef (CCC 35)
        ('\u{0327}', 202), // cedilla (CCC 202)
        ('\u{031B}', 216), // horn (CCC 216)
        ('\u{0323}', 220), // dot below (CCC 220)
        ('\u{0300}', 230), // grave (CCC 230)
        ('\u{0301}', 230), // acute (CCC 230)
        ('\u{0302}', 230), // circumflex (CCC 230)
        ('\u{0345}', 240), // iota subscript (CCC 240)
        ('\u{0303}', 230), // tilde (CCC 230)
    ];

    // Build input in reverse order (worst case)
    let mut input = String::from("a");
    for i in 0..54 {
        let (ch, _) = marks_by_ccc[(54 - 1 - i) % marks_by_ccc.len()];
        input.push(ch);
    }
    assert_all_forms_match_icu("long-54-multi-ccc-bands", &input);
}

// ===========================================================================
// 7. Worst-case CCC ordering (marks in reverse CCC order)
// ===========================================================================

#[test]
fn worst_case_reverse_ccc_order() {
    // Marks in strictly decreasing CCC order: 240, 230, 220, 202, 216, 1
    // After canonical ordering sort: 1, 202, 216, 220, 230, 240
    let input = "a\u{0345}\u{0300}\u{0323}\u{0327}\u{031B}\u{0334}";
    assert_all_forms_match_icu("worst-case-reverse-6-marks", input);
}

#[test]
fn worst_case_interleaved_ascending_descending() {
    // Interleaved pattern: high, low, high, low, ...
    // CCC: 230, 1, 220, 7, 202, 9, 240
    let input = "a\u{0300}\u{0334}\u{0323}\u{093C}\u{0327}\u{094D}\u{0345}";
    assert_all_forms_match_icu("worst-case-interleaved", input);
}

#[test]
fn worst_case_all_distinct_ccc_values() {
    // One mark from each major CCC group, in reverse order.
    // CCC 240: U+0345 iota subscript
    // CCC 234: U+035D double breve
    // CCC 233: U+0362 double rightwards arrow below
    // CCC 232: U+0350 right arrowhead above
    // CCC 230: U+0300 grave
    // CCC 226: U+1D165 (CCC 216 actually -- let's use real chars)
    // CCC 222: U+0339 right half ring below
    // CCC 220: U+0323 dot below
    // CCC 218: U+031C left half ring below (CCC 220 actually)
    // CCC 202: U+0327 cedilla
    // CCC 9:   U+094D virama
    // CCC 1:   U+0334 tilde overlay
    //
    // Put in reverse CCC order for maximum reordering.
    let input = "a\u{0345}\u{035D}\u{0362}\u{0350}\u{0300}\u{0323}\u{0327}\u{094D}\u{0334}";
    assert_all_forms_match_icu("worst-case-many-distinct-ccc", input);
}

#[test]
fn worst_case_duplicate_ccc_values_reversed() {
    // Same CCC value appears multiple times, in various positions among
    // different CCC values. Tests that stable sort keeps same-CCC marks
    // in original relative order even when heavily interleaved.
    //
    // Sequence: acute(230) dot-below(220) grave(230) cedilla(202) circumflex(230) ogonek(202)
    // After stable sort by CCC: cedilla(202) ogonek(202) dot-below(220) acute(230) grave(230) circumflex(230)
    let input = "e\u{0301}\u{0323}\u{0300}\u{0327}\u{0302}\u{0328}";
    assert_all_forms_match_icu("worst-case-duplicates-interleaved", input);
}

// ===========================================================================
// 8. Cross-form validation: NFD (sort only) vs NFC (sort + compose)
// ===========================================================================

#[test]
fn cross_form_nfd_vs_nfc_cedilla_acute() {
    // e + cedilla (CCC 202) + acute (CCC 230)
    // NFD: e + cedilla + acute (already sorted)
    // NFC: should compose e + cedilla into U+0229, then add acute
    let input = "e\u{0327}\u{0301}";
    assert_nfd_matches_icu("cross-form-nfd-cedilla-acute", input);
    assert_nfc_matches_icu("cross-form-nfc-cedilla-acute", input);

    // Reversed: e + acute (CCC 230) + cedilla (CCC 202)
    // After canonical reordering: e + cedilla + acute (same as above)
    let input_rev = "e\u{0301}\u{0327}";
    assert_nfd_matches_icu("cross-form-nfd-acute-cedilla", input_rev);
    assert_nfc_matches_icu("cross-form-nfc-acute-cedilla", input_rev);
}

#[test]
fn cross_form_blocking_by_intervening_mark() {
    // CCC blocking: e + cedilla (CCC 202) + dot below (CCC 220) + acute (CCC 230)
    // The cedilla (202) comes first. In NFC, e + cedilla composes to U+0229.
    // Then dot below (220) + acute (230) remain as combining marks.
    let input = "e\u{0327}\u{0323}\u{0301}";
    assert_all_forms_match_icu("cross-form-blocking-cedilla-dot-acute", input);

    // Now: e + dot below (220) + cedilla (202) + acute (230)
    // After sort: cedilla (202) + dot below (220) + acute (230)
    // Same result as above.
    let input2 = "e\u{0323}\u{0327}\u{0301}";
    assert_all_forms_match_icu("cross-form-blocking-reordered", input2);
}

// ===========================================================================
// 9. Additional edge cases: overlay marks (CCC 1), nukta (CCC 7), etc.
// ===========================================================================

#[test]
fn overlay_marks_ccc_1() {
    // CCC 1 marks should sort before all other non-zero CCC marks.
    // U+0334 COMBINING TILDE OVERLAY (CCC 1)
    // U+0335 COMBINING SHORT STROKE OVERLAY (CCC 1)
    // U+0338 COMBINING LONG SOLIDUS OVERLAY (CCC 1)
    //
    // Put a CCC 230 mark first, then CCC 1 marks.
    let input = "a\u{0301}\u{0334}\u{0335}\u{0338}";
    assert_all_forms_match_icu("overlay-ccc-1-after-230", input);
}

#[test]
fn nukta_ccc_7() {
    // Devanagari Ka (U+0915) + Nukta (U+093C, CCC 7) + Virama (U+094D, CCC 9)
    // Already in order, should be stable.
    let input = "\u{0915}\u{093C}\u{094D}";
    assert_all_forms_match_icu("nukta-virama-sorted", input);

    // Reversed: Ka + Virama (CCC 9) + Nukta (CCC 7) -> should reorder to Nukta then Virama
    let input_rev = "\u{0915}\u{094D}\u{093C}";
    assert_all_forms_match_icu("nukta-virama-reversed", input_rev);
}

#[test]
fn kana_voicing_ccc_8() {
    // Hiragana Ka (U+304B) + Combining Dakuten (U+3099, CCC 8)
    // This should compose in NFC to U+304C (Ga).
    let input = "\u{304B}\u{3099}";
    assert_all_forms_match_icu("kana-voicing-dakuten", input);

    // Hiragana Ha (U+306F) + Combining Handakuten (U+309A, CCC 8)
    // Should compose in NFC to U+3071 (Pa).
    let input2 = "\u{306F}\u{309A}";
    assert_all_forms_match_icu("kana-voicing-handakuten", input2);
}

#[test]
fn iota_subscript_ccc_240() {
    // Iota subscript (U+0345, CCC 240) is the highest CCC value.
    // It should always sort last among combining marks.
    //
    // Alpha (U+0391) + iota subscript (240) + grave (230) + smooth breathing (230 for U+0313)
    let input = "\u{0391}\u{0345}\u{0300}\u{0313}";
    assert_all_forms_match_icu("iota-subscript-sorts-last", input);

    // Greek lowercase alpha with marks in sorted CCC order
    // smooth breathing (U+0313, CCC 230) + grave (U+0300, CCC 230) + iota subscript (U+0345, CCC 240)
    let input2 = "\u{03B1}\u{0313}\u{0300}\u{0345}";
    assert_all_forms_match_icu("greek-alpha-breathing-grave-iota", input2);
}

// ===========================================================================
// 10. Hebrew and Arabic mixed together
// ===========================================================================

#[test]
fn hebrew_arabic_marks_on_same_base() {
    // Stress test: put Hebrew and Arabic combining marks on a single base.
    // This is linguistically unusual but must sort correctly.
    // Hebrew Sheva (CCC 10) + Arabic Fatha (CCC 30) + Arabic Shadda (CCC 33) + above grave (CCC 230)
    let input = "a\u{0300}\u{0651}\u{064E}\u{05B0}";
    assert_all_forms_match_icu("hebrew-arabic-mixed-on-latin", input);
}

// ===========================================================================
// 11. Multiple starters interspersed with combining sequences
// ===========================================================================

#[test]
fn multiple_clusters_with_marks() {
    // Several base characters each followed by combining marks in various orders.
    // Each cluster's marks should be sorted independently.
    let input = "a\u{0302}\u{0327}b\u{0323}\u{0301}c\u{0345}\u{0300}\u{0334}";
    assert_all_forms_match_icu("multi-cluster-with-marks", input);
}

#[test]
fn alternating_starters_and_marks() {
    // Starter, mark, starter, mark, ... pattern
    // Each mark sequence is length 1, so no sorting needed, but composition may apply.
    let input = "a\u{0301}b\u{0302}c\u{0303}d\u{0327}e\u{0323}";
    assert_all_forms_match_icu("alternating-starter-mark", input);
}

// ===========================================================================
// 12. Edge: empty and single-character inputs
// ===========================================================================

#[test]
fn empty_input() {
    assert_all_forms_match_icu("empty", "");
}

#[test]
fn single_combining_mark_no_base() {
    // A combining mark without a base character.
    let input = "\u{0301}";
    assert_all_forms_match_icu("lone-acute", input);

    // Multiple combining marks without a base.
    let input2 = "\u{0323}\u{0301}\u{0327}";
    assert_all_forms_match_icu("multiple-marks-no-base", input2);
}

// ===========================================================================
// 13. Stress: repeated base+marks pattern at scale
// ===========================================================================

#[test]
fn repeated_cluster_stress() {
    // 100 repetitions of a base + 3 marks in reverse CCC order.
    // This tests sustained correctness over a long string.
    let cluster = "e\u{0301}\u{0323}\u{0327}"; // acute(230) dot-below(220) cedilla(202)
    let input: String = cluster.repeat(100);
    assert_all_forms_match_icu("repeated-cluster-100x", &input);
}

// ===========================================================================
// 14. Combining marks from supplementary planes
// ===========================================================================

#[test]
fn supplementary_plane_combining_marks() {
    // Musical combining marks from the SMP (Supplementary Multilingual Plane).
    // U+1D165 MUSICAL SYMBOL COMBINING STEM (CCC 216)
    // U+1D166 MUSICAL SYMBOL COMBINING SPRECHGESANG STEM (CCC 216)
    // U+1D167 MUSICAL SYMBOL COMBINING TREMOLO-1 (CCC 1)
    // U+1D16D MUSICAL SYMBOL COMBINING AUGMENTATION DOT (CCC 226)

    // Musical void notehead + combining marks in reverse CCC order
    let input = "\u{1D157}\u{1D16D}\u{1D165}\u{1D167}";
    assert_all_forms_match_icu("musical-supplementary-marks", input);
}

// ===========================================================================
// 15. CCC 200 and 202 (attached below left / attached below)
// ===========================================================================

#[test]
fn attached_below_marks() {
    // U+0327 COMBINING CEDILLA (CCC 202)
    // U+0328 COMBINING OGONEK (CCC 202)
    // Both CCC 202: test stability.
    let input = "a\u{0327}\u{0328}";
    assert_all_forms_match_icu("attached-below-cedilla-ogonek", input);

    // Reversed
    let input_rev = "a\u{0328}\u{0327}";
    assert_all_forms_match_icu("attached-below-ogonek-cedilla", input_rev);
}

// ===========================================================================
// 16. Hebrew cantillation marks (CCC 220-230 range accents)
// ===========================================================================

#[test]
fn hebrew_cantillation_marks() {
    // Hebrew has a rich set of cantillation marks (te'amim) with various CCC values.
    // U+0591 ETNAHTA (CCC 220)
    // U+0592 SEGOL (CCC 230)
    // U+0593 SHALSHELET (CCC 230)
    // U+0594 ZAQEF QATAN (CCC 230)
    // U+0596 TIPEHA (CCC 220)
    // U+059A YETIV (CCC 222)
    //
    // Mix of CCC 220, 222, 230 on a Hebrew letter.
    let input = "\u{05D0}\u{0594}\u{0593}\u{0592}\u{059A}\u{0596}\u{0591}";
    assert_all_forms_match_icu("hebrew-cantillation-mixed", input);
}

// ===========================================================================
// 17. Regression guard: exactly INLINE_CAP marks with same CCC
// ===========================================================================

#[test]
fn exactly_inline_cap_same_ccc() {
    // Exactly 18 marks (INLINE_CAP) all with CCC 230.
    // This is the boundary case where inline storage is full but overflow
    // is NOT triggered.
    let marks: &[char] = &[
        '\u{0300}', '\u{0301}', '\u{0302}', '\u{0303}', '\u{0304}', '\u{0305}', '\u{0306}',
        '\u{0307}', '\u{0308}', '\u{030B}', '\u{030C}', '\u{030D}', '\u{030E}', '\u{030F}',
        '\u{0310}', '\u{0311}', '\u{0312}', '\u{0313}',
    ];
    assert_eq!(marks.len(), 18);

    let mut input = String::from("a");
    for &m in marks {
        input.push(m);
    }
    assert_all_forms_match_icu("exactly-inline-cap-18-same-ccc", &input);
}
