// tests/api.rs
use std::borrow::Cow;

#[test]
fn test_nfc_constructor_exists() {
    let norm = simd_normalizer::nfc();
    let result = norm.normalize("hello");
    assert_eq!(result, "hello");
}

#[test]
fn test_nfd_constructor_exists() {
    let norm = simd_normalizer::nfd();
    let result = norm.normalize("hello");
    assert_eq!(result, "hello");
}

#[test]
fn test_nfkc_constructor_exists() {
    let norm = simd_normalizer::nfkc();
    let result = norm.normalize("hello");
    assert_eq!(result, "hello");
}

#[test]
fn test_nfkd_constructor_exists() {
    let norm = simd_normalizer::nfkd();
    let result = norm.normalize("hello");
    assert_eq!(result, "hello");
}

#[test]
fn test_ascii_returns_borrowed() {
    let input = "The quick brown fox jumps over the lazy dog.";
    let result = simd_normalizer::nfc().normalize(input);
    match &result {
        Cow::Borrowed(s) => assert!(core::ptr::eq(*s, input)),
        Cow::Owned(_) => panic!("expected Cow::Borrowed for pure ASCII"),
    }
}

#[test]
fn test_trait_on_str() {
    use simd_normalizer::UnicodeNormalization;

    let input = "\u{00C5}\u{03A9}";
    let nfc_result = input.nfc();
    let nfd_result = input.nfd();
    let _nfkc_result = input.nfkc();
    let _nfkd_result = input.nfkd();

    assert_eq!(&*nfc_result, input);
    assert_ne!(&*nfd_result, input);
    assert!(input.is_nfc());
    assert!(!nfd_result.is_nfc());

    assert_eq!(&*input.nfc(), &*simd_normalizer::nfc().normalize(input));
    assert_eq!(&*input.nfd(), &*simd_normalizer::nfd().normalize(input));
    assert_eq!(&*input.nfkc(), &*simd_normalizer::nfkc().normalize(input));
    assert_eq!(&*input.nfkd(), &*simd_normalizer::nfkd().normalize(input));
}

#[test]
fn test_is_normalized_enum_exposed() {
    use simd_normalizer::IsNormalized;
    let _yes = IsNormalized::Yes;
    let _no = IsNormalized::No;
    let _maybe = IsNormalized::Maybe;
}

#[test]
fn test_normalize_to_buffer() {
    let norm = simd_normalizer::nfc();
    let mut buf = String::new();
    let was_normalized = norm.normalize_to("\u{0041}\u{030A}", &mut buf);
    assert!(!was_normalized);
    assert_eq!(buf, "\u{00C5}");

    buf.clear();
    let was_normalized = norm.normalize_to("hello", &mut buf);
    assert!(was_normalized);
    assert_eq!(buf, "hello");
}

#[test]
fn test_quick_check_method() {
    use simd_normalizer::IsNormalized;
    let norm = simd_normalizer::nfc();
    let result = norm.quick_check("hello world");
    assert_eq!(result, IsNormalized::Yes);
}

// =========================================================================
// normalize_to() for NFD, NFKC, NFKD
// =========================================================================

/// Test inputs used across normalize_to tests.
const TEST_ASCII: &str = "hello world";
const TEST_COMBINING: &str = "\u{0041}\u{030A}"; // A + combining ring above
const TEST_HANGUL_JAMO: &str = "\u{1100}\u{1161}"; // Hangul L + V (composes to 가)
const TEST_CJK_COMPAT: &str = "\u{2F800}"; // CJK compat ideograph
const TEST_EMOJI_ZWJ: &str = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}"; // family emoji
const TEST_PRECOMPOSED: &str = "\u{00C5}"; // Å (precomposed)
const TEST_COMPAT_LIGATURE: &str = "\u{FB01}"; // fi ligature

/// Macro to verify normalize_to produces the same output as normalize for a
/// given normalizer constructor and a variety of inputs.
macro_rules! assert_normalize_to_matches {
    ($constructor:expr, $label:expr) => {{
        let inputs: &[&str] = &[
            TEST_ASCII,
            TEST_COMBINING,
            TEST_HANGUL_JAMO,
            TEST_CJK_COMPAT,
            TEST_EMOJI_ZWJ,
            TEST_PRECOMPOSED,
            TEST_COMPAT_LIGATURE,
            "",         // empty string
            "Z",        // single ASCII
            "\u{0300}", // lone combining grave accent
        ];
        for &input in inputs {
            let norm = $constructor;
            let expected = norm.normalize(input);
            let mut buf = String::new();
            let was_normalized = norm.normalize_to(input, &mut buf);
            assert_eq!(
                buf, &*expected,
                concat!($label, ": normalize_to output mismatch for input {:?}"),
                input
            );
            // was_normalized should be true iff normalize returned Cow::Borrowed
            let is_borrowed = matches!(norm.normalize(input), Cow::Borrowed(_));
            assert_eq!(
                was_normalized, is_borrowed,
                concat!(
                    $label,
                    ": normalize_to return value mismatch for input {:?}"
                ),
                input
            );
        }
    }};
}

#[test]
fn test_normalize_to_nfd() {
    assert_normalize_to_matches!(simd_normalizer::nfd(), "NFD");
}

#[test]
fn test_normalize_to_nfkc() {
    assert_normalize_to_matches!(simd_normalizer::nfkc(), "NFKC");
}

#[test]
fn test_normalize_to_nfkd() {
    assert_normalize_to_matches!(simd_normalizer::nfkd(), "NFKD");
}

#[test]
fn test_normalize_to_nfc_extended() {
    // The existing test only covered two inputs; run the full suite for NFC too.
    assert_normalize_to_matches!(simd_normalizer::nfc(), "NFC");
}

// Spot-check specific expected values for normalize_to.

#[test]
fn test_normalize_to_nfd_specific_values() {
    let nfd = simd_normalizer::nfd();
    let mut buf = String::new();

    // Precomposed Å -> A + ring above
    nfd.normalize_to(TEST_PRECOMPOSED, &mut buf);
    assert_eq!(buf, "\u{0041}\u{030A}");

    // fi ligature is canonical-unchanged in NFD (only decomposes in NFKD)
    buf.clear();
    nfd.normalize_to(TEST_COMPAT_LIGATURE, &mut buf);
    assert_eq!(buf, "\u{FB01}");
}

#[test]
fn test_normalize_to_nfkc_specific_values() {
    let nfkc = simd_normalizer::nfkc();
    let mut buf = String::new();

    // fi ligature decomposes to "fi" in NFKC
    nfkc.normalize_to(TEST_COMPAT_LIGATURE, &mut buf);
    assert_eq!(buf, "fi");

    // Precomposed Å stays Å in NFKC
    buf.clear();
    nfkc.normalize_to(TEST_PRECOMPOSED, &mut buf);
    assert_eq!(buf, "\u{00C5}");
}

#[test]
fn test_normalize_to_nfkd_specific_values() {
    let nfkd = simd_normalizer::nfkd();
    let mut buf = String::new();

    // fi ligature decomposes to "fi" in NFKD
    nfkd.normalize_to(TEST_COMPAT_LIGATURE, &mut buf);
    assert_eq!(buf, "fi");

    // Precomposed Å decomposes to A + ring above in NFKD
    buf.clear();
    nfkd.normalize_to(TEST_PRECOMPOSED, &mut buf);
    assert_eq!(buf, "\u{0041}\u{030A}");
}

#[test]
fn test_normalize_to_appends_to_existing_buffer() {
    // Verify normalize_to *appends* rather than replacing.
    let nfd = simd_normalizer::nfd();
    let mut buf = String::from("prefix:");
    nfd.normalize_to("hello", &mut buf);
    assert_eq!(buf, "prefix:hello");
}

// =========================================================================
// quick_check() via struct methods for all four forms
// =========================================================================

#[test]
fn test_quick_check_nfd_yes() {
    use simd_normalizer::IsNormalized;
    let nfd = simd_normalizer::nfd();
    assert_eq!(nfd.quick_check("hello"), IsNormalized::Yes);
    // Decomposed form should also be Yes for NFD
    assert_eq!(nfd.quick_check("\u{0041}\u{030A}"), IsNormalized::Yes);
}

#[test]
fn test_quick_check_nfd_no() {
    use simd_normalizer::IsNormalized;
    let nfd = simd_normalizer::nfd();
    // Precomposed Å is NOT in NFD
    assert_eq!(nfd.quick_check("\u{00C5}"), IsNormalized::No);
    // Hangul syllable is NOT in NFD (it decomposes)
    assert_eq!(nfd.quick_check("\u{AC00}"), IsNormalized::No);
}

#[test]
fn test_quick_check_nfc_yes() {
    use simd_normalizer::IsNormalized;
    let nfc = simd_normalizer::nfc();
    assert_eq!(nfc.quick_check("hello"), IsNormalized::Yes);
    // Precomposed Å is in NFC
    assert_eq!(nfc.quick_check("\u{00C5}"), IsNormalized::Yes);
}

#[test]
fn test_quick_check_nfc_not_yes() {
    use simd_normalizer::IsNormalized;
    let nfc = simd_normalizer::nfc();
    // A + cedilla + ring above triggers CCC reordering -> No
    let result = nfc.quick_check("\u{0041}\u{0327}\u{030A}");
    assert!(
        result == IsNormalized::No || result == IsNormalized::Maybe,
        "Expected No or Maybe for CCC-reordered sequence, got {:?}",
        result
    );
}

#[test]
fn test_quick_check_nfc_maybe() {
    use simd_normalizer::IsNormalized;
    let nfc = simd_normalizer::nfc();
    // Combining grave accent (U+0300) has NFC_QC=Maybe
    let result = nfc.quick_check("\u{0300}");
    assert!(
        result == IsNormalized::Maybe || result == IsNormalized::No,
        "Expected Maybe or No for lone combining grave, got {:?}",
        result
    );
}

#[test]
fn test_quick_check_nfkc_yes() {
    use simd_normalizer::IsNormalized;
    let nfkc = simd_normalizer::nfkc();
    assert_eq!(nfkc.quick_check("hello"), IsNormalized::Yes);
    assert_eq!(nfkc.quick_check("\u{00C5}"), IsNormalized::Yes);
}

#[test]
fn test_quick_check_nfkc_no() {
    use simd_normalizer::IsNormalized;
    let nfkc = simd_normalizer::nfkc();
    // fi ligature is NOT in NFKC (it decomposes to "fi")
    assert_eq!(nfkc.quick_check("\u{FB01}"), IsNormalized::No);
    // NBSP is NOT in NFKC (it maps to SPACE)
    assert_eq!(nfkc.quick_check("\u{00A0}"), IsNormalized::No);
}

#[test]
fn test_quick_check_nfkd_yes() {
    use simd_normalizer::IsNormalized;
    let nfkd = simd_normalizer::nfkd();
    assert_eq!(nfkd.quick_check("hello"), IsNormalized::Yes);
    // Decomposed A + ring is in NFKD
    assert_eq!(nfkd.quick_check("\u{0041}\u{030A}"), IsNormalized::Yes);
}

#[test]
fn test_quick_check_nfkd_no() {
    use simd_normalizer::IsNormalized;
    let nfkd = simd_normalizer::nfkd();
    // fi ligature is NOT in NFKD
    assert_eq!(nfkd.quick_check("\u{FB01}"), IsNormalized::No);
    // Precomposed Å is NOT in NFKD (it decomposes)
    assert_eq!(nfkd.quick_check("\u{00C5}"), IsNormalized::No);
    // Hangul syllable is NOT in NFKD
    assert_eq!(nfkd.quick_check("\u{AC00}"), IsNormalized::No);
}

#[test]
fn test_quick_check_empty_string_all_forms() {
    use simd_normalizer::IsNormalized;
    assert_eq!(simd_normalizer::nfc().quick_check(""), IsNormalized::Yes);
    assert_eq!(simd_normalizer::nfd().quick_check(""), IsNormalized::Yes);
    assert_eq!(simd_normalizer::nfkc().quick_check(""), IsNormalized::Yes);
    assert_eq!(simd_normalizer::nfkd().quick_check(""), IsNormalized::Yes);
}

// =========================================================================
// Default::default() for all four normalizer structs
// =========================================================================

#[test]
fn test_default_nfc() {
    let default_norm = simd_normalizer::NfcNormalizer;
    let explicit_norm = simd_normalizer::nfc();
    let inputs = [
        TEST_ASCII,
        TEST_COMBINING,
        TEST_PRECOMPOSED,
        TEST_HANGUL_JAMO,
        TEST_COMPAT_LIGATURE,
    ];
    for input in &inputs {
        assert_eq!(
            &*default_norm.normalize(input),
            &*explicit_norm.normalize(input),
            "NfcNormalizer::default() vs nfc() mismatch for {:?}",
            input
        );
    }
}

#[test]
fn test_default_nfd() {
    let default_norm = simd_normalizer::NfdNormalizer;
    let explicit_norm = simd_normalizer::nfd();
    let inputs = [
        TEST_ASCII,
        TEST_COMBINING,
        TEST_PRECOMPOSED,
        TEST_HANGUL_JAMO,
        TEST_COMPAT_LIGATURE,
    ];
    for input in &inputs {
        assert_eq!(
            &*default_norm.normalize(input),
            &*explicit_norm.normalize(input),
            "NfdNormalizer::default() vs nfd() mismatch for {:?}",
            input
        );
    }
}

#[test]
fn test_default_nfkc() {
    let default_norm = simd_normalizer::NfkcNormalizer;
    let explicit_norm = simd_normalizer::nfkc();
    let inputs = [
        TEST_ASCII,
        TEST_COMBINING,
        TEST_PRECOMPOSED,
        TEST_HANGUL_JAMO,
        TEST_COMPAT_LIGATURE,
    ];
    for input in &inputs {
        assert_eq!(
            &*default_norm.normalize(input),
            &*explicit_norm.normalize(input),
            "NfkcNormalizer::default() vs nfkc() mismatch for {:?}",
            input
        );
    }
}

#[test]
fn test_default_nfkd() {
    let default_norm = simd_normalizer::NfkdNormalizer;
    let explicit_norm = simd_normalizer::nfkd();
    let inputs = [
        TEST_ASCII,
        TEST_COMBINING,
        TEST_PRECOMPOSED,
        TEST_HANGUL_JAMO,
        TEST_COMPAT_LIGATURE,
    ];
    for input in &inputs {
        assert_eq!(
            &*default_norm.normalize(input),
            &*explicit_norm.normalize(input),
            "NfkdNormalizer::default() vs nfkd() mismatch for {:?}",
            input
        );
    }
}

// =========================================================================
// IsNormalized enum -- verify derives (Debug, Clone, Copy, PartialEq, Eq)
// =========================================================================

#[test]
fn test_is_normalized_debug() {
    use simd_normalizer::IsNormalized;
    // Debug derive: format should produce the variant name.
    assert_eq!(format!("{:?}", IsNormalized::Yes), "Yes");
    assert_eq!(format!("{:?}", IsNormalized::No), "No");
    assert_eq!(format!("{:?}", IsNormalized::Maybe), "Maybe");
}

#[test]
fn test_is_normalized_clone() {
    use simd_normalizer::IsNormalized;
    let original = IsNormalized::Maybe;
    let cloned = original;
    assert_eq!(original, cloned);
}

#[test]
fn test_is_normalized_copy() {
    use simd_normalizer::IsNormalized;
    let a = IsNormalized::Yes;
    let b = a; // Copy
    let c = a; // still usable after move if Copy
    assert_eq!(b, c);
    assert_eq!(a, IsNormalized::Yes); // a is still valid (Copy)
}

#[test]
fn test_is_normalized_eq() {
    use simd_normalizer::IsNormalized;
    // PartialEq + Eq
    assert_eq!(IsNormalized::Yes, IsNormalized::Yes);
    assert_eq!(IsNormalized::No, IsNormalized::No);
    assert_eq!(IsNormalized::Maybe, IsNormalized::Maybe);
    assert_ne!(IsNormalized::Yes, IsNormalized::No);
    assert_ne!(IsNormalized::Yes, IsNormalized::Maybe);
    assert_ne!(IsNormalized::No, IsNormalized::Maybe);
}
