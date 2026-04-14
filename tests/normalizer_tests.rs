//! Comprehensive tests for the normalizer core.

extern crate alloc;

use alloc::borrow::Cow;
use alloc::string::String;
use simd_normalizer::normalizer::{NfcNormalizer, NfdNormalizer, NfkcNormalizer, NfkdNormalizer};

// ============================================================================
// Helper
// ============================================================================

#[allow(clippy::ptr_arg)]
fn is_borrowed(cow: &Cow<'_, str>) -> bool {
    matches!(cow, Cow::Borrowed(_))
}

// ============================================================================
// 1. Empty string
// ============================================================================

#[test]
fn empty_string_nfc() {
    let n = NfcNormalizer;
    let result = n.normalize("");
    assert_eq!(&*result, "");
    assert!(is_borrowed(&result));
}

#[test]
fn empty_string_nfd() {
    let n = NfdNormalizer;
    let result = n.normalize("");
    assert_eq!(&*result, "");
    assert!(is_borrowed(&result));
}

#[test]
fn empty_string_nfkc() {
    let n = NfkcNormalizer;
    let result = n.normalize("");
    assert_eq!(&*result, "");
    assert!(is_borrowed(&result));
}

#[test]
fn empty_string_nfkd() {
    let n = NfkdNormalizer;
    let result = n.normalize("");
    assert_eq!(&*result, "");
    assert!(is_borrowed(&result));
}

// ============================================================================
// 2. ASCII passthrough -- short (<64 bytes)
// ============================================================================

#[test]
fn ascii_short_nfc_borrowed() {
    let n = NfcNormalizer;
    let input = "Hello, world!";
    let result = n.normalize(input);
    assert_eq!(&*result, input);
    assert!(is_borrowed(&result));
}

#[test]
fn ascii_short_nfd_borrowed() {
    let n = NfdNormalizer;
    let input = "Hello, world!";
    let result = n.normalize(input);
    assert_eq!(&*result, input);
    assert!(is_borrowed(&result));
}

// ============================================================================
// 3. ASCII passthrough -- long (>=64 bytes)
// ============================================================================

#[test]
fn ascii_long_nfc_borrowed() {
    let n = NfcNormalizer;
    let input = "A".repeat(200);
    let result = n.normalize(&input);
    assert_eq!(&*result, &*input);
    assert!(is_borrowed(&result));
}

#[test]
fn ascii_long_nfd_borrowed() {
    let n = NfdNormalizer;
    let input = "The quick brown fox jumps over the lazy dog. ".repeat(5);
    let result = n.normalize(&input);
    assert_eq!(&*result, &*input);
    assert!(is_borrowed(&result));
}

#[test]
fn ascii_long_nfkc_borrowed() {
    let n = NfkcNormalizer;
    let input = "ABCDEFGHIJ".repeat(20);
    let result = n.normalize(&input);
    assert_eq!(&*result, &*input);
    assert!(is_borrowed(&result));
}

#[test]
fn ascii_long_nfkd_borrowed() {
    let n = NfkdNormalizer;
    let input = "0123456789".repeat(20);
    let result = n.normalize(&input);
    assert_eq!(&*result, &*input);
    assert!(is_borrowed(&result));
}

// ============================================================================
// 4. NFD decomposition
// ============================================================================

#[test]
fn nfd_e_acute() {
    let n = NfdNormalizer;
    let result = n.normalize("\u{00E9}");
    assert_eq!(&*result, "e\u{0301}");
}

#[test]
fn nfd_cafe() {
    let n = NfdNormalizer;
    let result = n.normalize("caf\u{00E9}");
    assert_eq!(&*result, "cafe\u{0301}");
}

#[test]
fn nfd_a_grave() {
    let n = NfdNormalizer;
    let result = n.normalize("\u{00C0}");
    assert_eq!(&*result, "A\u{0300}");
}

#[test]
fn nfd_o_diaeresis() {
    let n = NfdNormalizer;
    let result = n.normalize("\u{00F6}");
    assert_eq!(&*result, "o\u{0308}");
}

// ============================================================================
// 5. NFC composition
// ============================================================================

#[test]
fn nfc_compose_e_acute() {
    let n = NfcNormalizer;
    let result = n.normalize("e\u{0301}");
    assert_eq!(&*result, "\u{00E9}");
}

#[test]
fn nfc_already_composed() {
    let n = NfcNormalizer;
    let result = n.normalize("\u{00E9}");
    // Should remain the same (already NFC).
    assert_eq!(&*result, "\u{00E9}");
}

#[test]
fn nfc_compose_a_ring() {
    let n = NfcNormalizer;
    let result = n.normalize("a\u{030A}");
    assert_eq!(&*result, "\u{00E5}");
}

// ============================================================================
// 6. Hangul LV decomposition
// ============================================================================

#[test]
fn nfd_hangul_lv() {
    let n = NfdNormalizer;
    let result = n.normalize("\u{AC00}");
    assert_eq!(&*result, "\u{1100}\u{1161}");
}

// ============================================================================
// 7. Hangul LVT decomposition
// ============================================================================

#[test]
fn nfd_hangul_lvt() {
    let n = NfdNormalizer;
    let result = n.normalize("\u{AC01}");
    assert_eq!(&*result, "\u{1100}\u{1161}\u{11A8}");
}

// ============================================================================
// 8. Hangul NFC composition
// ============================================================================

#[test]
fn nfc_hangul_lv_composition() {
    let n = NfcNormalizer;
    let result = n.normalize("\u{1100}\u{1161}");
    assert_eq!(&*result, "\u{AC00}");
}

#[test]
fn nfc_hangul_lvt_composition() {
    let n = NfcNormalizer;
    let result = n.normalize("\u{1100}\u{1161}\u{11A8}");
    assert_eq!(&*result, "\u{AC01}");
}

// ============================================================================
// 9. NFKD compatibility decomposition
// ============================================================================

#[test]
fn nfkd_fi_ligature() {
    let n = NfkdNormalizer;
    let result = n.normalize("\u{FB01}");
    assert_eq!(&*result, "fi");
}

#[test]
fn nfkd_superscript_two() {
    let n = NfkdNormalizer;
    let result = n.normalize("\u{00B2}");
    assert_eq!(&*result, "2");
}

#[test]
fn nfkd_fullwidth_a() {
    let n = NfkdNormalizer;
    let result = n.normalize("\u{FF21}");
    assert_eq!(&*result, "A");
}

// ============================================================================
// 10. NFKC compatibility composition
// ============================================================================

#[test]
fn nfkc_fi_ligature() {
    let n = NfkcNormalizer;
    let result = n.normalize("\u{FB01}");
    assert_eq!(&*result, "fi");
}

#[test]
fn nfkc_superscript_two() {
    let n = NfkcNormalizer;
    let result = n.normalize("\u{00B2}");
    assert_eq!(&*result, "2");
}

#[test]
fn nfkc_fullwidth_a() {
    let n = NfkcNormalizer;
    let result = n.normalize("\u{FF21}");
    assert_eq!(&*result, "A");
}

// ============================================================================
// 11. CCC reordering
// ============================================================================

#[test]
fn nfc_ccc_reorder() {
    // U+006F 'o' + U+0308 diaeresis (CCC 230) + U+0327 cedilla (CCC 202)
    // In NFD canonical ordering, cedilla (202) comes before diaeresis (230).
    // NFC reorders to: o + cedilla(202) + diaeresis(230).
    // Then composition: o + cedilla -> no composition, but o + diaeresis -> U+00F6 (o-diaeresis).
    // Cedilla (CCC 202) does NOT block diaeresis (CCC 230) since 202 < 230.
    // Result: U+00F6 (o-diaeresis) + U+0327 (cedilla).
    let n = NfcNormalizer;
    let input = "o\u{0308}\u{0327}";
    let result = n.normalize(input);
    let chars: Vec<char> = result.chars().collect();
    assert_eq!(chars.len(), 2);
    assert_eq!(chars[0], '\u{00F6}'); // o-diaeresis
    assert_eq!(chars[1], '\u{0327}'); // cedilla
}

#[test]
fn nfd_ccc_reorder() {
    let n = NfdNormalizer;
    let input = "o\u{0308}\u{0327}";
    let result = n.normalize(input);
    let chars: Vec<char> = result.chars().collect();
    assert_eq!(chars[0], 'o');
    assert_eq!(chars[1], '\u{0327}'); // cedilla CCC 202
    assert_eq!(chars[2], '\u{0308}'); // diaeresis CCC 230
}

// ============================================================================
// 12. Chunk boundary: multi-byte char straddling 64-byte boundary
// ============================================================================

#[test]
fn chunk_boundary_multibyte() {
    // Create exactly 63 ASCII bytes then a 2-byte char, so it straddles position 63-64.
    let n = NfcNormalizer;
    let prefix = "a".repeat(63);
    let input = format!("{}\u{00E9}", prefix); // 63 + 2 = 65 bytes
    let result = n.normalize(&input);
    // NFC: e-acute is already NFC, should remain as-is.
    assert_eq!(&*result, &*input);
}

#[test]
fn chunk_boundary_decompose() {
    // 62 ASCII bytes + U+00C0 (A-grave, 2 bytes) at position 62-63, exactly hitting chunk boundary.
    let n = NfdNormalizer;
    let prefix = "b".repeat(62);
    let input = format!("{}\u{00C0}", prefix); // 62 + 2 = 64 bytes
    let result = n.normalize(&input);
    let expected = format!("{}A\u{0300}", prefix);
    assert_eq!(&*result, &*expected);
}

// ============================================================================
// 13. Long combining sequences spanning chunks
// ============================================================================

#[test]
fn long_combining_sequence() {
    let n = NfcNormalizer;
    // Starter 'a' followed by many combining marks.
    let mut input = String::from("a");
    for _ in 0..20 {
        input.push('\u{0301}'); // combining acute (CCC 230)
    }
    let result = n.normalize(&input);
    // First acute should compose with 'a' -> a-acute, rest remain (blocked by same CCC).
    let chars: Vec<char> = result.chars().collect();
    assert_eq!(chars[0], '\u{00E1}'); // a-acute
    assert_eq!(chars.len(), 20); // 1 composed + 19 remaining acutes
}

// ============================================================================
// 14. normalize_to tests
// ============================================================================

#[test]
fn normalize_to_appends() {
    let n = NfcNormalizer;
    let mut out = String::from("prefix:");
    n.normalize_to("e\u{0301}", &mut out);
    assert_eq!(out, "prefix:\u{00E9}");
}

#[test]
fn normalize_to_already_normalized() {
    let n = NfcNormalizer;
    let mut out = String::new();
    let was_normalized = n.normalize_to("Hello", &mut out);
    assert!(was_normalized);
    assert_eq!(out, "Hello");
}

#[test]
fn normalize_to_not_normalized() {
    let n = NfcNormalizer;
    let mut out = String::new();
    let was_normalized = n.normalize_to("e\u{0301}", &mut out);
    assert!(!was_normalized);
    assert_eq!(out, "\u{00E9}");
}

// ============================================================================
// 15. Large input (10KB): exercises SIMD path
// ============================================================================

#[test]
fn large_ascii_input() {
    let n = NfcNormalizer;
    let input = "x".repeat(10240);
    let result = n.normalize(&input);
    assert_eq!(&*result, &*input);
    assert!(is_borrowed(&result));
}

#[test]
fn large_mixed_input() {
    let n = NfdNormalizer;
    // ~10KB of mixed ASCII and non-ASCII.
    let chunk = "Hello caf\u{00E9} world! ";
    let input = chunk.repeat(500);
    let result = n.normalize(&input);
    // Verify it contains decomposed form.
    assert!(result.contains("cafe\u{0301}"));
    // Verify length is reasonable (decomposed form is slightly longer).
    assert!(result.len() >= input.len());
}

// ============================================================================
// 16. Multiple starters with marks
// ============================================================================

#[test]
fn nfd_multiple_precomposed() {
    let n = NfdNormalizer;
    let result = n.normalize("\u{00E9}\u{00F6}");
    assert_eq!(&*result, "e\u{0301}o\u{0308}");
}

#[test]
fn nfc_multiple_decomposed() {
    let n = NfcNormalizer;
    let result = n.normalize("e\u{0301}o\u{0308}");
    assert_eq!(&*result, "\u{00E9}\u{00F6}");
}

// ============================================================================
// 17. Mixed scripts: ASCII + Latin + CJK + emoji
// ============================================================================

#[test]
fn mixed_scripts_nfc() {
    let n = NfcNormalizer;
    let input = "Hello \u{00E9} \u{4E16}\u{754C} \u{1F600}";
    let result = n.normalize(input);
    // All these are already in NFC.
    assert_eq!(&*result, input);
}

#[test]
fn mixed_scripts_nfd() {
    let n = NfdNormalizer;
    let input = "Hello \u{00E9} \u{4E16}\u{754C} \u{1F600}";
    let result = n.normalize(input);
    // Only e-acute should decompose.
    assert_eq!(&*result, "Hello e\u{0301} \u{4E16}\u{754C} \u{1F600}");
}

// ============================================================================
// 18. is_normalized tests
// ============================================================================

#[test]
fn is_normalized_nfc_ascii() {
    let n = NfcNormalizer;
    assert!(n.is_normalized("Hello"));
}

#[test]
fn is_normalized_nfc_precomposed() {
    let n = NfcNormalizer;
    assert!(n.is_normalized("\u{00E9}"));
}

#[test]
fn is_normalized_nfc_rejects_nfd() {
    let n = NfcNormalizer;
    assert!(!n.is_normalized("e\u{0301}"));
}

#[test]
fn is_normalized_nfd_decomposed() {
    let n = NfdNormalizer;
    assert!(n.is_normalized("e\u{0301}"));
}

#[test]
fn is_normalized_nfd_rejects_nfc() {
    let n = NfdNormalizer;
    assert!(!n.is_normalized("\u{00E9}"));
}

// ============================================================================
// 19. Roundtrip: NFC(NFD(x)) == NFC(x) for various inputs
// ============================================================================

#[test]
fn roundtrip_nfc_nfd() {
    let nfc = NfcNormalizer;
    let nfd = NfdNormalizer;

    let inputs = &[
        "Hello",
        "\u{00E9}",
        "caf\u{00E9}",
        "\u{AC00}",
        "\u{AC01}",
        "e\u{0301}o\u{0308}",
        "\u{1100}\u{1161}\u{11A8}",
    ];

    for &input in inputs {
        let nfd_form = nfd.normalize(input);
        let nfc_form = nfc.normalize(input);
        let nfc_of_nfd = nfc.normalize(&nfd_form);
        assert_eq!(
            &*nfc_form, &*nfc_of_nfd,
            "NFC(NFD(x)) != NFC(x) for input: {:?}",
            input
        );
    }
}

// ============================================================================
// 20. Single character edge cases
// ============================================================================

#[test]
fn single_ascii_char() {
    let n = NfcNormalizer;
    let result = n.normalize("A");
    assert_eq!(&*result, "A");
    assert!(is_borrowed(&result));
}

#[test]
fn single_combining_mark() {
    // A leading combining mark with no preceding starter.
    let n = NfcNormalizer;
    let result = n.normalize("\u{0301}");
    assert_eq!(&*result, "\u{0301}");
}

// ============================================================================
// 21. Hangul in SIMD-length context
// ============================================================================

#[test]
fn hangul_in_long_string_nfd() {
    let n = NfdNormalizer;
    let prefix = "a".repeat(100);
    let input = format!("{}\u{AC00}", prefix);
    let result = n.normalize(&input);
    let expected = format!("{}\u{1100}\u{1161}", prefix);
    assert_eq!(&*result, &*expected);
}

#[test]
fn hangul_in_long_string_nfc() {
    let n = NfcNormalizer;
    let prefix = "a".repeat(100);
    let input = format!("{}\u{1100}\u{1161}", prefix);
    let result = n.normalize(&input);
    let expected = format!("{}\u{AC00}", prefix);
    assert_eq!(&*result, &*expected);
}
