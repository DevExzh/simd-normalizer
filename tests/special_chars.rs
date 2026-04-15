// tests/special_chars.rs
//
// Tests normalization behavior for special Unicode characters with unique
// semantics: variation selectors, BOM, directional marks, zero-width
// characters, combining grapheme joiner, non-character code points, tag
// characters, interlinear annotation anchors, and the replacement character.
//
// Each category is tested across all four normalization forms (NFC, NFD,
// NFKC, NFKD), including `is_normalized` checks and differential validation
// against icu_normalizer.

use icu_normalizer::{ComposingNormalizerBorrowed, DecomposingNormalizerBorrowed};
use simd_normalizer::UnicodeNormalization;

// ---------------------------------------------------------------------------
// Helpers: assert all 4 forms + cross-validate against icu_normalizer
// ---------------------------------------------------------------------------

/// Assert that normalization in the given form produces `expected`, and that
/// icu_normalizer agrees.
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

/// Assert that all four forms preserve the input unchanged, plus cross-validate.
fn assert_all_forms_unchanged(input: &str) {
    assert_nfc(input, input);
    assert_nfd(input, input);
    assert_nfkc(input, input);
    assert_nfkd(input, input);
}

/// Assert is_normalized returns the expected value for all four forms, and
/// that icu_normalizer agrees.
fn assert_is_normalized_all(input: &str, nfc: bool, nfd: bool, nfkc: bool, nfkd: bool) {
    assert_eq!(
        input.is_nfc(),
        nfc,
        "is_nfc mismatch for {:?}: expected {}",
        input,
        nfc
    );
    assert_eq!(
        input.is_nfd(),
        nfd,
        "is_nfd mismatch for {:?}: expected {}",
        input,
        nfd
    );
    assert_eq!(
        input.is_nfkc(),
        nfkc,
        "is_nfkc mismatch for {:?}: expected {}",
        input,
        nfkc
    );
    assert_eq!(
        input.is_nfkd(),
        nfkd,
        "is_nfkd mismatch for {:?}: expected {}",
        input,
        nfkd
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
// 1. Variation Selectors
// ===========================================================================

#[test]
fn variation_selectors_in_isolation() {
    // VS1 (U+FE00) alone
    assert_all_forms_unchanged("\u{FE00}");
    // VS16 / emoji variation selector (U+FE0F) alone
    assert_all_forms_unchanged("\u{FE0F}");
}

#[test]
fn variation_selectors_after_base_character() {
    // CJK character U+9089 + VS1 (U+FE00) — used for CJK glyph variants
    let cjk_vs = "\u{9089}\u{FE00}";
    assert_all_forms_unchanged(cjk_vs);
    assert_is_normalized_all(cjk_vs, true, true, true, true);

    // Snowman (U+2603) + emoji VS16 (U+FE0F)
    let snowman_emoji = "\u{2603}\u{FE0F}";
    assert_all_forms_unchanged(snowman_emoji);
    assert_is_normalized_all(snowman_emoji, true, true, true, true);
}

#[test]
fn variation_selectors_supplementary() {
    // VS17 (U+E0100) — first supplementary variation selector
    let supp_vs17 = "\u{9089}\u{E0100}";
    assert_all_forms_unchanged(supp_vs17);
    assert_is_normalized_all(supp_vs17, true, true, true, true);

    // VS256 (U+E01EF) — last supplementary variation selector
    let supp_vs256 = "\u{9089}\u{E01EF}";
    assert_all_forms_unchanged(supp_vs256);
    assert_is_normalized_all(supp_vs256, true, true, true, true);

    // Supplementary VS in isolation
    assert_all_forms_unchanged("\u{E0100}");
    assert_all_forms_unchanged("\u{E01EF}");
}

#[test]
fn variation_selectors_in_text() {
    // Embedded between ASCII
    let mixed = "Hello\u{9089}\u{FE00}World";
    assert_all_forms_unchanged(mixed);

    // Between CJK characters
    let cjk_seq = "\u{4E00}\u{FE01}\u{4E8C}\u{FE02}\u{4E09}";
    assert_all_forms_unchanged(cjk_seq);
}

// ===========================================================================
// 2. BOM (U+FEFF)
// ===========================================================================

#[test]
fn bom_as_first_char() {
    let bom_first = "\u{FEFF}Hello";
    assert_all_forms_unchanged(bom_first);
    assert_is_normalized_all(bom_first, true, true, true, true);
}

#[test]
fn bom_in_middle() {
    let bom_mid = "He\u{FEFF}llo";
    assert_all_forms_unchanged(bom_mid);
    assert_is_normalized_all(bom_mid, true, true, true, true);
}

#[test]
fn bom_multiple() {
    let multi_bom = "\u{FEFF}\u{FEFF}\u{FEFF}";
    assert_all_forms_unchanged(multi_bom);
    assert_is_normalized_all(multi_bom, true, true, true, true);
}

#[test]
fn bom_with_non_ascii() {
    // BOM + CJK + combining sequence
    let bom_complex = "\u{FEFF}\u{4E00}a\u{0308}";
    // NFC composes a + combining diaeresis -> U+00E4
    assert_nfc(bom_complex, "\u{FEFF}\u{4E00}\u{00E4}");
    assert_nfd(bom_complex, "\u{FEFF}\u{4E00}a\u{0308}");
    assert_nfkc(bom_complex, "\u{FEFF}\u{4E00}\u{00E4}");
    assert_nfkd(bom_complex, "\u{FEFF}\u{4E00}a\u{0308}");
}

// ===========================================================================
// 3. Directional Marks
// ===========================================================================

#[test]
fn directional_marks_in_isolation() {
    // LRM (U+200E), RLM (U+200F)
    assert_all_forms_unchanged("\u{200E}");
    assert_all_forms_unchanged("\u{200F}");

    // Embedding marks: LRE, RLE, PDF, LRO, RLO
    assert_all_forms_unchanged("\u{202A}");
    assert_all_forms_unchanged("\u{202B}");
    assert_all_forms_unchanged("\u{202C}");
    assert_all_forms_unchanged("\u{202D}");
    assert_all_forms_unchanged("\u{202E}");
}

#[test]
fn directional_marks_is_normalized() {
    let lrm = "\u{200E}";
    assert_is_normalized_all(lrm, true, true, true, true);

    let rlm = "\u{200F}";
    assert_is_normalized_all(rlm, true, true, true, true);

    let lre = "\u{202A}";
    assert_is_normalized_all(lre, true, true, true, true);
}

#[test]
fn directional_marks_in_text() {
    // LRM/RLM embedded in text
    let mixed = "Hello\u{200E}World\u{200F}!";
    assert_all_forms_unchanged(mixed);

    // Bidi embedding around Arabic text
    let bidi = "\u{202A}Arabic\u{202C}English";
    assert_all_forms_unchanged(bidi);

    // LRO around mixed text
    let lro = "\u{202D}mixed\u{200E}text\u{202C}";
    assert_all_forms_unchanged(lro);
}

#[test]
fn directional_marks_with_combining() {
    // Directional marks + combining sequences: the marks should pass through
    // and not interfere with composition.
    let input = "a\u{200E}\u{0308}";
    // LRM (U+200E) has CCC=0, so it is a starter and blocks composition of
    // 'a' + U+0308. In NFC the combining diaeresis cannot reach back past
    // the LRM to compose with 'a'.
    assert_nfc(input, "a\u{200E}\u{0308}");
    assert_nfd(input, "a\u{200E}\u{0308}");
}

// ===========================================================================
// 4. Zero-Width Characters
// ===========================================================================

#[test]
fn zero_width_space_isolation() {
    // ZWS (U+200B)
    assert_all_forms_unchanged("\u{200B}");
    assert_is_normalized_all("\u{200B}", true, true, true, true);
}

#[test]
fn zwnj_isolation() {
    // ZWNJ (U+200C)
    assert_all_forms_unchanged("\u{200C}");
    assert_is_normalized_all("\u{200C}", true, true, true, true);
}

#[test]
fn zwj_isolation() {
    // ZWJ (U+200D)
    assert_all_forms_unchanged("\u{200D}");
    assert_is_normalized_all("\u{200D}", true, true, true, true);
}

#[test]
fn zwnj_blocks_composition() {
    // "e" + ZWNJ + combining acute (U+0301)
    // ZWNJ has CCC=0 (it is a starter), so it blocks composition.
    // In NFC, the ZWNJ prevents 'e' + U+0301 from composing into U+00E9.
    let input = "e\u{200C}\u{0301}";
    assert_nfc(input, "e\u{200C}\u{0301}");
    assert_nfd(input, "e\u{200C}\u{0301}");
    assert_nfkc(input, "e\u{200C}\u{0301}");
    assert_nfkd(input, "e\u{200C}\u{0301}");
    assert_is_normalized_all(input, true, true, true, true);
}

#[test]
fn zwj_blocks_composition() {
    // ZWJ (U+200D) also has CCC=0, so it too blocks composition.
    let input = "e\u{200D}\u{0301}";
    assert_nfc(input, "e\u{200D}\u{0301}");
    assert_nfd(input, "e\u{200D}\u{0301}");
    assert_nfkc(input, "e\u{200D}\u{0301}");
    assert_nfkd(input, "e\u{200D}\u{0301}");
    assert_is_normalized_all(input, true, true, true, true);
}

#[test]
fn zwj_does_not_affect_already_composed() {
    // ZWJ between two already-composed characters should not change anything.
    let input = "\u{00E9}\u{200D}\u{00E0}";
    // In NFD these decompose: e + acute, ZWJ, a + grave
    assert_nfd(input, "e\u{0301}\u{200D}a\u{0300}");
    // In NFC the decomposed forms compose back on each side of ZWJ
    assert_nfc(input, "\u{00E9}\u{200D}\u{00E0}");
}

#[test]
fn zero_width_chars_in_text() {
    let text = "con\u{200B}nect\u{200C}ed\u{200D}text";
    assert_all_forms_unchanged(text);
}

// ===========================================================================
// 5. Combining Grapheme Joiner (U+034F)
// ===========================================================================

#[test]
fn cgj_in_isolation() {
    assert_all_forms_unchanged("\u{034F}");
    assert_is_normalized_all("\u{034F}", true, true, true, true);
}

#[test]
fn cgj_blocks_composition() {
    // CGJ (U+034F) has CCC=0, so it is a starter and blocks composition.
    // Without CGJ: "e" + U+0301 -> NFC U+00E9.
    // With CGJ:    "e" + CGJ + U+0301 -> composition blocked, stays as-is.
    let input = "e\u{034F}\u{0301}";
    assert_nfc(input, "e\u{034F}\u{0301}");
    assert_nfd(input, "e\u{034F}\u{0301}");
    assert_nfkc(input, "e\u{034F}\u{0301}");
    assert_nfkd(input, "e\u{034F}\u{0301}");
    assert_is_normalized_all(input, true, true, true, true);
}

#[test]
fn cgj_between_composable_pair() {
    // Without CGJ: 'A' + U+030A (combining ring above) composes to U+00C5 in NFC
    let without_cgj = "A\u{030A}";
    assert_nfc(without_cgj, "\u{00C5}");

    // With CGJ: composition is blocked
    let with_cgj = "A\u{034F}\u{030A}";
    assert_nfc(with_cgj, "A\u{034F}\u{030A}");
    assert_nfd(with_cgj, "A\u{034F}\u{030A}");
}

#[test]
fn cgj_in_text() {
    let text = "Hello\u{034F}World";
    assert_all_forms_unchanged(text);
}

// ===========================================================================
// 6. Non-Character Code Points
// ===========================================================================

#[test]
fn non_characters_fdd0_fdef() {
    // U+FDD0..U+FDEF are non-characters but valid Rust chars
    for cp in 0xFDD0..=0xFDEFu32 {
        let ch = char::from_u32(cp).expect("FDD0-FDEF should be valid Rust chars");
        let s = String::from(ch);
        assert_all_forms_unchanged(&s);
    }
}

#[test]
fn non_characters_fffe_ffff() {
    // U+FFFE and U+FFFF
    let fffe = "\u{FFFE}";
    assert_all_forms_unchanged(fffe);
    assert_is_normalized_all(fffe, true, true, true, true);

    let ffff = "\u{FFFF}";
    assert_all_forms_unchanged(ffff);
    assert_is_normalized_all(ffff, true, true, true, true);
}

#[test]
fn non_characters_supplementary() {
    // U+1FFFE, U+1FFFF
    let s1fffe = "\u{1FFFE}";
    assert_all_forms_unchanged(s1fffe);
    assert_is_normalized_all(s1fffe, true, true, true, true);

    let s1ffff = "\u{1FFFF}";
    assert_all_forms_unchanged(s1ffff);
    assert_is_normalized_all(s1ffff, true, true, true, true);
}

#[test]
fn non_characters_last_plane() {
    // U+10FFFE, U+10FFFF (last valid Unicode code points, both non-characters)
    let s10fffe = "\u{10FFFE}";
    assert_all_forms_unchanged(s10fffe);
    assert_is_normalized_all(s10fffe, true, true, true, true);

    let s10ffff = "\u{10FFFF}";
    assert_all_forms_unchanged(s10ffff);
    assert_is_normalized_all(s10ffff, true, true, true, true);
}

#[test]
fn non_characters_in_text() {
    // Non-characters embedded in otherwise normal text should not cause panics
    let text = "Hello\u{FDD0}World\u{FFFE}!\u{10FFFF}";
    assert_all_forms_unchanged(text);
}

#[test]
fn non_characters_all_planes() {
    // Test the xFFFE/xFFFF non-characters for planes 0-16
    for plane in 0..=16u32 {
        let base = plane * 0x10000;
        let fffe = char::from_u32(base + 0xFFFE);
        let ffff = char::from_u32(base + 0xFFFF);
        if let Some(ch) = fffe {
            let s = String::from(ch);
            assert_all_forms_unchanged(&s);
        }
        if let Some(ch) = ffff {
            let s = String::from(ch);
            assert_all_forms_unchanged(&s);
        }
    }
}

// ===========================================================================
// 7. Tag Characters
// ===========================================================================

#[test]
fn tag_characters_isolation() {
    // Language tag (U+E0001) — deprecated but valid
    assert_all_forms_unchanged("\u{E0001}");
    assert_is_normalized_all("\u{E0001}", true, true, true, true);

    // Tag space (U+E0020)
    assert_all_forms_unchanged("\u{E0020}");

    // Tag tilde (U+E007E)
    assert_all_forms_unchanged("\u{E007E}");

    // Cancel tag (U+E007F)
    assert_all_forms_unchanged("\u{E007F}");
    assert_is_normalized_all("\u{E007F}", true, true, true, true);
}

#[test]
fn tag_characters_sequence() {
    // A tag sequence spelling "en-US" using tag characters (E0065=e, E006E=n, ...)
    let tag_seq = "\u{E0001}\u{E0065}\u{E006E}\u{E002D}\u{E0055}\u{E0053}\u{E007F}";
    assert_all_forms_unchanged(tag_seq);
    assert_is_normalized_all(tag_seq, true, true, true, true);
}

#[test]
fn tag_characters_in_text() {
    let text = "Flag\u{E0001}\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F}here";
    assert_all_forms_unchanged(text);
}

#[test]
fn tag_characters_range() {
    // Test all tag characters U+E0020..U+E007F
    for cp in 0xE0020..=0xE007Fu32 {
        let ch = char::from_u32(cp).expect("tag characters should be valid");
        let s = String::from(ch);
        assert_all_forms_unchanged(&s);
    }
}

// ===========================================================================
// 8. Interlinear Annotation Anchors
// ===========================================================================

#[test]
fn annotation_anchors_isolation() {
    // IAA (U+FFF9), IAS (U+FFFA), IAT (U+FFFB)
    assert_all_forms_unchanged("\u{FFF9}");
    assert_all_forms_unchanged("\u{FFFA}");
    assert_all_forms_unchanged("\u{FFFB}");

    assert_is_normalized_all("\u{FFF9}", true, true, true, true);
    assert_is_normalized_all("\u{FFFA}", true, true, true, true);
    assert_is_normalized_all("\u{FFFB}", true, true, true, true);
}

#[test]
fn annotation_anchors_sequence() {
    // Typical usage: IAA base IAS annotation IAT
    let annotated = "\u{FFF9}base\u{FFFA}annotation\u{FFFB}";
    assert_all_forms_unchanged(annotated);
    assert_is_normalized_all(annotated, true, true, true, true);
}

#[test]
fn annotation_anchors_in_text() {
    let text = "Some \u{FFF9}text\u{FFFA}ruby\u{FFFB} here";
    assert_all_forms_unchanged(text);
}

// ===========================================================================
// 9. Replacement Character (U+FFFD)
// ===========================================================================

#[test]
fn replacement_char_isolation() {
    assert_all_forms_unchanged("\u{FFFD}");
    assert_is_normalized_all("\u{FFFD}", true, true, true, true);
}

#[test]
fn replacement_char_multiple() {
    let multi = "\u{FFFD}\u{FFFD}\u{FFFD}";
    assert_all_forms_unchanged(multi);
    assert_is_normalized_all(multi, true, true, true, true);
}

#[test]
fn replacement_char_in_text() {
    let text = "Hello\u{FFFD}World";
    assert_all_forms_unchanged(text);

    // Mixed with CJK
    let cjk_text = "\u{4E00}\u{FFFD}\u{4E8C}";
    assert_all_forms_unchanged(cjk_text);
}

#[test]
fn replacement_char_with_combining() {
    // Replacement char followed by combining marks — they should attach to it
    let input = "\u{FFFD}\u{0301}\u{0302}";
    assert_all_forms_unchanged(input);
    assert_is_normalized_all(input, true, true, true, true);
}

// ===========================================================================
// 10. Cross-category combinations
// ===========================================================================

#[test]
fn bom_then_variation_selector() {
    let input = "\u{FEFF}\u{9089}\u{FE00}";
    assert_all_forms_unchanged(input);
}

#[test]
fn directional_marks_with_tag_characters() {
    let input = "\u{200E}\u{E0001}\u{E0065}\u{E007F}\u{200F}";
    assert_all_forms_unchanged(input);
}

#[test]
fn replacement_char_with_annotation_anchors() {
    let input = "\u{FFF9}\u{FFFD}\u{FFFA}replaced\u{FFFB}";
    assert_all_forms_unchanged(input);
}

#[test]
fn zwj_between_emoji_with_variation_selectors() {
    // Family-style ZWJ sequence: person + ZWJ + heart + VS16 + ZWJ + person
    let input = "\u{1F468}\u{200D}\u{2764}\u{FE0F}\u{200D}\u{1F468}";
    assert_all_forms_unchanged(input);
    assert_is_normalized_all(input, true, true, true, true);
}

#[test]
fn all_zero_width_chars_in_sequence() {
    let input = "\u{200B}\u{200C}\u{200D}\u{034F}\u{FEFF}";
    assert_all_forms_unchanged(input);
    assert_is_normalized_all(input, true, true, true, true);
}

#[test]
fn non_characters_with_directional_marks() {
    let input = "\u{200E}\u{FFFE}\u{200F}\u{FFFF}";
    assert_all_forms_unchanged(input);
}

#[test]
fn special_chars_surrounding_composable_pair() {
    // Verify that special chars before/after a composable pair do not
    // interfere with composition.
    // BOM + 'a' + combining diaeresis + FFFD
    let input = "\u{FEFF}a\u{0308}\u{FFFD}";
    assert_nfc(input, "\u{FEFF}\u{00E4}\u{FFFD}");
    assert_nfd(input, "\u{FEFF}a\u{0308}\u{FFFD}");
    assert_nfkc(input, "\u{FEFF}\u{00E4}\u{FFFD}");
    assert_nfkd(input, "\u{FEFF}a\u{0308}\u{FFFD}");
}

#[test]
fn long_run_of_special_chars() {
    // Stress test: 256 BOMs followed by a composable pair
    let mut input = String::new();
    for _ in 0..256 {
        input.push('\u{FEFF}');
    }
    input.push('o');
    input.push('\u{0308}'); // combining diaeresis

    let mut expected_nfc = String::new();
    for _ in 0..256 {
        expected_nfc.push('\u{FEFF}');
    }
    expected_nfc.push('\u{00F6}'); // o + diaeresis -> U+00F6

    assert_nfc(&input, &expected_nfc);
    assert_nfd(&input, &input); // NFD: already decomposed
}

#[test]
fn special_chars_do_not_interfere_with_hangul() {
    // Hangul composition with ZWS in between should not be affected
    // (ZWS has CCC=0 so it's a starter and blocks Hangul composition)
    // L + ZWS + V: composition blocked
    let input = "\u{1100}\u{200B}\u{1161}";
    assert_nfc(input, "\u{1100}\u{200B}\u{1161}");
    assert_nfd(input, "\u{1100}\u{200B}\u{1161}");

    // Without ZWS: L + V composes
    let lv = "\u{1100}\u{1161}";
    assert_nfc(lv, "\u{AC00}");
}
