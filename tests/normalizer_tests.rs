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

// ============================================================================
// 22. Long combining mark sequences (>32) and starter-less input
//
// These tests exercise the fallback path in compose_combining_sequence_into()
// which uses Vec allocation when the combining sequence exceeds 32 marks
// (the bitmask capacity). All results are cross-checked against icu_normalizer.
// ============================================================================

/// Format code points for diagnostics.
fn codepoints_debug(s: &str) -> String {
    s.chars()
        .map(|c| format!("U+{:04X}", c as u32))
        .collect::<Vec<_>>()
        .join(" ")
}

/// ICU4X NFC reference.
fn icu_nfc(s: &str) -> String {
    icu_normalizer::ComposingNormalizerBorrowed::new_nfc()
        .normalize(s)
        .into_owned()
}

/// ICU4X NFD reference.
fn icu_nfd(s: &str) -> String {
    icu_normalizer::DecomposingNormalizerBorrowed::new_nfd()
        .normalize(s)
        .into_owned()
}

/// ICU4X NFKC reference.
fn icu_nfkc(s: &str) -> String {
    icu_normalizer::ComposingNormalizerBorrowed::new_nfkc()
        .normalize(s)
        .into_owned()
}

/// ICU4X NFKD reference.
fn icu_nfkd(s: &str) -> String {
    icu_normalizer::DecomposingNormalizerBorrowed::new_nfkd()
        .normalize(s)
        .into_owned()
}

/// Assert our result matches ICU4X with detailed diagnostics on failure.
fn assert_matches_icu(form: &str, input: &str, ours: &str, reference: &str) {
    assert_eq!(
        ours,
        reference,
        "\n{form} divergence from icu_normalizer!\
         \n  input len: {ilen} chars\
         \n  ours  len: {olen} chars\
         \n  ref   len: {rlen} chars\
         \n  first 10 input cps: {input_cps}\
         \n  first 10 ours  cps: {ours_cps}\
         \n  first 10 ref   cps: {ref_cps}",
        form = form,
        ilen = input.chars().count(),
        olen = ours.chars().count(),
        rlen = reference.chars().count(),
        input_cps = input.chars().take(10).map(|c| format!("U+{:04X}", c as u32)).collect::<Vec<_>>().join(" "),
        ours_cps = ours.chars().take(10).map(|c| format!("U+{:04X}", c as u32)).collect::<Vec<_>>().join(" "),
        ref_cps = reference.chars().take(10).map(|c| format!("U+{:04X}", c as u32)).collect::<Vec<_>>().join(" "),
    );
}

// --- Test 22a: NFC with exactly 33 combining marks (just over the bitmask threshold) ---

#[test]
fn nfc_33_combining_marks_fallback_path() {
    let nfc = NfcNormalizer;
    let nfd = NfdNormalizer;

    // Build: 'a' + 33 combining marks with varied CCC values.
    // Use a mix of CCC 202 (cedilla, ogonek), CCC 220 (below marks), and CCC 230 (above marks)
    // to exercise CCC reordering through the fallback path.
    let marks: &[char] = &[
        '\u{0327}', // cedilla, CCC=202
        '\u{0328}', // ogonek, CCC=202
        '\u{0323}', // dot below, CCC=220
        '\u{0330}', // tilde below, CCC=220
        '\u{0331}', // macron below, CCC=220
        '\u{0332}', // low line, CCC=220
        '\u{0300}', // grave, CCC=230
        '\u{0301}', // acute, CCC=230
        '\u{0302}', // circumflex, CCC=230
        '\u{0303}', // tilde, CCC=230
        '\u{0304}', // macron, CCC=230
        '\u{0306}', // breve, CCC=230
        '\u{0307}', // dot above, CCC=230
        '\u{0308}', // diaeresis, CCC=230
        '\u{0309}', // hook above, CCC=230
        '\u{030A}', // ring above, CCC=230
        '\u{030B}', // double acute, CCC=230
        '\u{030C}', // caron, CCC=230
        '\u{0345}', // ypogegrammeni, CCC=240
        // Repeat from the beginning to get to 33
        '\u{0327}', // cedilla, CCC=202
        '\u{0328}', // ogonek, CCC=202
        '\u{0323}', // dot below, CCC=220
        '\u{0330}', // tilde below, CCC=220
        '\u{0331}', // macron below, CCC=220
        '\u{0332}', // low line, CCC=220
        '\u{0300}', // grave, CCC=230
        '\u{0301}', // acute, CCC=230
        '\u{0302}', // circumflex, CCC=230
        '\u{0303}', // tilde, CCC=230
        '\u{0304}', // macron, CCC=230
        '\u{0306}', // breve, CCC=230
        '\u{0307}', // dot above, CCC=230
        '\u{0308}', // diaeresis, CCC=230
    ];
    assert_eq!(marks.len(), 33, "must have exactly 33 combining marks");

    let mut input = String::from("a");
    for &m in marks {
        input.push(m);
    }

    // NFC
    let our_nfc_result = nfc.normalize(&input);
    let icu_nfc_result = icu_nfc(&input);
    assert_matches_icu("NFC-33marks", &input, &our_nfc_result, &icu_nfc_result);

    // NFD
    let our_nfd_result = nfd.normalize(&input);
    let icu_nfd_result = icu_nfd(&input);
    assert_matches_icu("NFD-33marks", &input, &our_nfd_result, &icu_nfd_result);
}

// --- Test 22b: NFC with 64 combining marks (deeply into fallback) ---

#[test]
fn nfc_64_combining_marks_deep_fallback() {
    let nfc = NfcNormalizer;
    let nfd = NfdNormalizer;

    // 64 combining marks with mixed CCC values, cycling through different categories.
    let mark_cycle: &[char] = &[
        '\u{0327}', // CCC=202
        '\u{0323}', // CCC=220
        '\u{0300}', // CCC=230
        '\u{0345}', // CCC=240
        '\u{0328}', // CCC=202
        '\u{0330}', // CCC=220
        '\u{0301}', // CCC=230
        '\u{0331}', // CCC=220
        '\u{0302}', // CCC=230
        '\u{0332}', // CCC=220
        '\u{0303}', // CCC=230
        '\u{0304}', // CCC=230
        '\u{0306}', // CCC=230
        '\u{0307}', // CCC=230
        '\u{0308}', // CCC=230
        '\u{0309}', // CCC=230
    ];

    let mut input = String::from("a");
    for i in 0..64 {
        input.push(mark_cycle[i % mark_cycle.len()]);
    }
    assert_eq!(input.chars().count(), 65, "1 starter + 64 marks");

    // NFC
    let our_nfc_result = nfc.normalize(&input);
    let icu_nfc_result = icu_nfc(&input);
    assert_matches_icu("NFC-64marks", &input, &our_nfc_result, &icu_nfc_result);

    // NFD
    let our_nfd_result = nfd.normalize(&input);
    let icu_nfd_result = icu_nfd(&input);
    assert_matches_icu("NFD-64marks", &input, &our_nfd_result, &icu_nfd_result);
}

// --- Test 22c: NFC with 100 combining marks ---

#[test]
fn nfc_100_combining_marks_extreme_fallback() {
    let nfc = NfcNormalizer;
    let nfd = NfdNormalizer;

    // 100 combining marks cycling through all four CCC tiers.
    let mark_cycle: &[char] = &[
        '\u{0327}', // CCC=202
        '\u{0328}', // CCC=202
        '\u{0323}', // CCC=220
        '\u{0330}', // CCC=220
        '\u{0331}', // CCC=220
        '\u{0332}', // CCC=220
        '\u{0300}', // CCC=230
        '\u{0301}', // CCC=230
        '\u{0302}', // CCC=230
        '\u{0303}', // CCC=230
        '\u{0304}', // CCC=230
        '\u{0306}', // CCC=230
        '\u{0307}', // CCC=230
        '\u{0308}', // CCC=230
        '\u{0309}', // CCC=230
        '\u{030A}', // CCC=230
        '\u{030B}', // CCC=230
        '\u{030C}', // CCC=230
        '\u{0345}', // CCC=240
    ];

    let mut input = String::from("e");
    for i in 0..100 {
        input.push(mark_cycle[i % mark_cycle.len()]);
    }
    assert_eq!(input.chars().count(), 101, "1 starter + 100 marks");

    // NFC
    let our_nfc_result = nfc.normalize(&input);
    let icu_nfc_result = icu_nfc(&input);
    assert_matches_icu("NFC-100marks", &input, &our_nfc_result, &icu_nfc_result);

    // NFD
    let our_nfd_result = nfd.normalize(&input);
    let icu_nfd_result = icu_nfd(&input);
    assert_matches_icu("NFD-100marks", &input, &our_nfd_result, &icu_nfd_result);
}

// --- Test 22d: Input of ONLY combining marks (no starter) ---

#[test]
fn only_combining_marks_no_starter_nfc() {
    let nfc = NfcNormalizer;
    let input = "\u{0300}\u{0301}\u{0327}";

    let our_result = nfc.normalize(input);
    let icu_result = icu_nfc(input);
    assert_matches_icu("NFC-no-starter", input, &our_result, &icu_result);

    // Must not panic, must produce non-empty output (the marks should pass through).
    assert!(!our_result.is_empty(), "output should not be empty for combining-only input");
}

#[test]
fn only_combining_marks_no_starter_nfd() {
    let nfd = NfdNormalizer;
    let input = "\u{0300}\u{0301}\u{0327}";

    let our_result = nfd.normalize(input);
    let icu_result = icu_nfd(input);
    assert_matches_icu("NFD-no-starter", input, &our_result, &icu_result);
    assert!(!our_result.is_empty());
}

#[test]
fn only_combining_marks_no_starter_nfkc() {
    let nfkc = NfkcNormalizer;
    let input = "\u{0300}\u{0301}\u{0327}";

    let our_result = nfkc.normalize(input);
    let icu_result = icu_nfkc(input);
    assert_matches_icu("NFKC-no-starter", input, &our_result, &icu_result);
    assert!(!our_result.is_empty());
}

#[test]
fn only_combining_marks_no_starter_nfkd() {
    let nfkd = NfkdNormalizer;
    let input = "\u{0300}\u{0301}\u{0327}";

    let our_result = nfkd.normalize(input);
    let icu_result = icu_nfkd(input);
    assert_matches_icu("NFKD-no-starter", input, &our_result, &icu_result);
    assert!(!our_result.is_empty());
}

#[test]
fn only_combining_marks_longer_sequence() {
    // A longer sequence of only combining marks (no starter at all).
    let nfc = NfcNormalizer;
    let nfd = NfdNormalizer;

    let mut input = String::new();
    let marks: &[char] = &[
        '\u{0300}', '\u{0301}', '\u{0302}', '\u{0303}', '\u{0304}',
        '\u{0327}', '\u{0328}', '\u{0323}', '\u{0330}', '\u{0345}',
    ];
    for &m in marks {
        input.push(m);
    }

    let our_nfc_result = nfc.normalize(&input);
    let icu_nfc_result = icu_nfc(&input);
    assert_matches_icu("NFC-no-starter-long", &input, &our_nfc_result, &icu_nfc_result);

    let our_nfd_result = nfd.normalize(&input);
    let icu_nfd_result = icu_nfd(&input);
    assert_matches_icu("NFD-no-starter-long", &input, &our_nfd_result, &icu_nfd_result);
}

// --- Test 22e: 33 marks with mixed composable/non-composable ---

#[test]
fn nfc_33_marks_mixed_composable_non_composable() {
    let nfc = NfcNormalizer;
    let nfd = NfdNormalizer;

    // Starter 'a' (U+0061) followed by 33 combining marks.
    // Some compose with 'a' (e.g. U+0301 acute -> U+00E1) and some don't
    // (e.g. U+0308 diaeresis composes with 'a' -> U+00E4, but is blocked by
    // earlier same-CCC marks).
    //
    // We intentionally put composable marks at different positions, including
    // marks with lower CCC values that don't compose, to test that:
    // 1. CCC reordering is correct
    // 2. Blocking detection works through the fallback path
    // 3. Composition happens where it should
    let marks: &[char] = &[
        // These two CCC=202 marks don't compose with 'a', but their lower CCC
        // means they don't block the CCC=230 marks from reaching the starter.
        '\u{0327}', // cedilla CCC=202 - does NOT compose with 'a'
        '\u{0328}', // ogonek CCC=202 - does NOT compose with 'a'
        // CCC=220 marks - don't compose with 'a'
        '\u{0323}', // dot below CCC=220
        '\u{0330}', // tilde below CCC=220
        '\u{0331}', // macron below CCC=220
        '\u{0332}', // low line CCC=220
        // CCC=230 marks - first one that composes with 'a' should win
        '\u{0301}', // acute CCC=230 -- composes with 'a' -> U+00E1 (if not blocked)
        '\u{0300}', // grave CCC=230 -- blocked by acute (same CCC)
        '\u{0302}', // circumflex CCC=230
        '\u{0303}', // tilde CCC=230
        '\u{0304}', // macron CCC=230
        '\u{0306}', // breve CCC=230
        '\u{0307}', // dot above CCC=230
        '\u{0308}', // diaeresis CCC=230
        '\u{0309}', // hook above CCC=230
        '\u{030A}', // ring above CCC=230
        '\u{030B}', // double acute CCC=230
        '\u{030C}', // caron CCC=230
        // CCC=240
        '\u{0345}', // ypogegrammeni CCC=240
        // More CCC=202 marks
        '\u{0327}', // cedilla CCC=202
        '\u{0328}', // ogonek CCC=202
        // More CCC=220 marks
        '\u{0323}', // dot below CCC=220
        '\u{0330}', // tilde below CCC=220
        '\u{0331}', // macron below CCC=220
        // More CCC=230 marks
        '\u{0300}', // grave CCC=230
        '\u{0301}', // acute CCC=230
        '\u{0302}', // circumflex CCC=230
        '\u{0303}', // tilde CCC=230
        '\u{0304}', // macron CCC=230
        '\u{0306}', // breve CCC=230
        '\u{0307}', // dot above CCC=230
        '\u{0308}', // diaeresis CCC=230
        '\u{0345}', // ypogegrammeni CCC=240
    ];
    assert_eq!(marks.len(), 33, "must have exactly 33 combining marks");

    let mut input = String::from("a");
    for &m in marks {
        input.push(m);
    }

    // NFC -- verify against ICU
    let our_nfc_result = nfc.normalize(&input);
    let icu_nfc_result = icu_nfc(&input);
    assert_matches_icu("NFC-33marks-mixed", &input, &our_nfc_result, &icu_nfc_result);

    // Verify that composition actually happened (the output should differ from
    // just stacking all marks on 'a').
    let nfc_chars: Vec<char> = our_nfc_result.chars().collect();
    // The first char should NOT be 'a' anymore (it should have composed with something).
    // After NFD decomposition + CCC sort, the cedilla (202) comes first, then dot below (220),
    // then acute (230). Since cedilla (202) doesn't compose with 'a', but acute (230)
    // can reach past it (lower CCC doesn't block), 'a' + acute should compose to U+00E1.
    // But actually in NFC, the input is first decomposed, then CCC-sorted, then composed.
    // Let's just verify the output is correct by checking it matches ICU.
    assert!(
        nfc_chars.len() < input.chars().count(),
        "NFC should compose at least one mark with the starter, \
         reducing char count. Got {} chars from {} input chars.\
         \n  output cps: {}",
        nfc_chars.len(),
        input.chars().count(),
        codepoints_debug(&our_nfc_result),
    );

    // NFD -- verify against ICU
    let our_nfd_result = nfd.normalize(&input);
    let icu_nfd_result = icu_nfd(&input);
    assert_matches_icu("NFD-33marks-mixed", &input, &our_nfd_result, &icu_nfd_result);

    // NFD output should be CCC-sorted: verify marks are in non-decreasing CCC order.
    let nfd_chars: Vec<char> = our_nfd_result.chars().collect();
    assert_eq!(nfd_chars[0], 'a', "NFD: starter should remain 'a'");
    let mut prev_ccc = 0u8;
    for &ch in &nfd_chars[1..] {
        let ccc = unicode_ccc_approximate(ch);
        assert!(
            ccc >= prev_ccc,
            "NFD CCC ordering violation: U+{:04X} (CCC={}) after CCC={}",
            ch as u32,
            ccc,
            prev_ccc,
        );
        prev_ccc = ccc;
    }
}

/// Approximate CCC lookup for test assertions.
/// Only covers the specific marks used in our tests.
fn unicode_ccc_approximate(ch: char) -> u8 {
    match ch {
        '\u{0327}' | '\u{0328}' => 202,
        '\u{0323}' | '\u{0330}' | '\u{0331}' | '\u{0332}' => 220,
        '\u{0300}' | '\u{0301}' | '\u{0302}' | '\u{0303}' | '\u{0304}'
        | '\u{0306}' | '\u{0307}' | '\u{0308}' | '\u{0309}' | '\u{030A}'
        | '\u{030B}' | '\u{030C}' => 230,
        '\u{0345}' => 240,
        _ => 0, // assume starter / CCC=0
    }
}

// --- Test 22f: Roundtrip and idempotency for long combining sequences ---

#[test]
fn long_combining_roundtrip_idempotency() {
    let nfc = NfcNormalizer;
    let nfd = NfdNormalizer;

    // Build a 50-mark input (well into fallback territory).
    let mark_cycle: &[char] = &[
        '\u{0327}', '\u{0323}', '\u{0300}', '\u{0345}',
        '\u{0328}', '\u{0330}', '\u{0301}', '\u{0331}',
    ];
    let mut input = String::from("o");
    for i in 0..50 {
        input.push(mark_cycle[i % mark_cycle.len()]);
    }

    // NFC should be idempotent: NFC(NFC(x)) == NFC(x)
    let nfc_once = nfc.normalize(&input).into_owned();
    let nfc_twice = nfc.normalize(&nfc_once).into_owned();
    assert_eq!(
        nfc_once, nfc_twice,
        "NFC is not idempotent for 50-mark input!\
         \n  NFC(x) cps: {}\
         \n  NFC(NFC(x)) cps: {}",
        codepoints_debug(&nfc_once),
        codepoints_debug(&nfc_twice),
    );

    // NFD should be idempotent: NFD(NFD(x)) == NFD(x)
    let nfd_once = nfd.normalize(&input).into_owned();
    let nfd_twice = nfd.normalize(&nfd_once).into_owned();
    assert_eq!(
        nfd_once, nfd_twice,
        "NFD is not idempotent for 50-mark input!\
         \n  NFD(x) cps: {}\
         \n  NFD(NFD(x)) cps: {}",
        codepoints_debug(&nfd_once),
        codepoints_debug(&nfd_twice),
    );

    // Roundtrip: NFC(NFD(x)) == NFC(x)
    let nfc_of_nfd = nfc.normalize(&nfd_once).into_owned();
    assert_eq!(
        nfc_once, nfc_of_nfd,
        "NFC(NFD(x)) != NFC(x) for 50-mark input!\
         \n  NFC(x) cps: {}\
         \n  NFC(NFD(x)) cps: {}",
        codepoints_debug(&nfc_once),
        codepoints_debug(&nfc_of_nfd),
    );
}
