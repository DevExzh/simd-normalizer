//! Cross-form invariant tests for Unicode normalization.
//!
//! Verifies algebraic relationships BETWEEN normalization forms using both
//! property-based testing (proptest) and deterministic tests with known
//! edge-case inputs.
//!
//! Invariants tested:
//!   1. NFKC implies NFC (NFKC is stricter)
//!   2. NFKD implies NFD (NFKD is stricter)
//!   3. NFKC(x) == NFC(NFKD(x)) (fundamental Unicode invariant)
//!   4. NFKD(x) == NFD(NFKC(x)) (dual invariant)
//!   5. quick_check / normalize agreement
//!   6. normalize_to matches normalize
//!   7. is_normalized cross-form consistency
//!   8. Decomposition subsumption
//!   9. Cross-validation with ICU4X

use icu_normalizer::{ComposingNormalizerBorrowed, DecomposingNormalizerBorrowed};
use proptest::prelude::*;
use simd_normalizer::IsNormalized;
use simd_normalizer::UnicodeNormalization;
use std::borrow::Cow;

// ---------------------------------------------------------------------------
// Strategy generators (same as tests/properties.rs)
// ---------------------------------------------------------------------------

/// Broad mix of Unicode scripts for general property testing.
fn unicode_string_strategy() -> impl Strategy<Value = String> {
    let ranges = prop::char::ranges(std::borrow::Cow::Borrowed(&[
        // ASCII
        '\u{0020}'..='\u{007E}',
        // Latin Extended-A / Extended-B
        '\u{0100}'..='\u{024F}',
        // Combining Diacritical Marks
        '\u{0300}'..='\u{036F}',
        // Cyrillic
        '\u{0400}'..='\u{04FF}',
        // Arabic
        '\u{0600}'..='\u{06FF}',
        // Devanagari
        '\u{0900}'..='\u{097F}',
        // Hangul Jamo
        '\u{1100}'..='\u{11FF}',
        // Hiragana
        '\u{3040}'..='\u{309F}',
        // CJK Unified Ideographs (small slice)
        '\u{4E00}'..='\u{4FFF}',
        // Hangul Syllables (small slice)
        '\u{AC00}'..='\u{D7A3}',
        // Emoticons
        '\u{1F600}'..='\u{1F64F}',
    ]));
    prop::collection::vec(ranges, 1..64).prop_map(|chars| chars.into_iter().collect::<String>())
}

/// Strategy that includes compatibility characters (ligatures, CJK compat,
/// fullwidth forms, superscripts, etc.) to exercise NFKC/NFKD differences.
fn compat_heavy_strategy() -> impl Strategy<Value = String> {
    let ranges = prop::char::ranges(std::borrow::Cow::Borrowed(&[
        // ASCII (baseline)
        '\u{0041}'..='\u{005A}',
        '\u{0061}'..='\u{007A}',
        // Latin Extended with precomposed chars
        '\u{00C0}'..='\u{00FF}',
        // Combining Diacritical Marks
        '\u{0300}'..='\u{036F}',
        // Superscripts and Subscripts
        '\u{2070}'..='\u{209F}',
        // Letterlike Symbols (includes Ohm, Kelvin, Angstrom)
        '\u{2100}'..='\u{214F}',
        // Number Forms (fractions)
        '\u{2150}'..='\u{218F}',
        // Enclosed Alphanumerics
        '\u{2460}'..='\u{24FF}',
        // CJK Compatibility
        '\u{3300}'..='\u{33FF}',
        // Alphabetic Presentation Forms (includes fi, fl ligatures)
        '\u{FB00}'..='\u{FB06}',
        // Halfwidth and Fullwidth Forms
        '\u{FF01}'..='\u{FF5E}',
        // Hangul Syllables (small slice)
        '\u{AC00}'..='\u{AC10}',
    ]));
    prop::collection::vec(ranges, 1..32).prop_map(|chars| chars.into_iter().collect::<String>())
}

/// Strategy biased toward supplementary-plane characters (planes 1-2).
#[allow(dead_code)]
fn supplementary_heavy_strategy() -> impl Strategy<Value = String> {
    let ranges = prop::char::ranges(std::borrow::Cow::Borrowed(&[
        // Musical Symbols (plane 1)
        '\u{1D100}'..='\u{1D1FF}',
        // Mathematical Alphanumeric Symbols (plane 1)
        '\u{1D400}'..='\u{1D7FF}',
        // Emoticons (plane 1)
        '\u{1F600}'..='\u{1F64F}',
        // CJK Unified Ideographs Extension B (plane 2, small slice)
        '\u{20000}'..='\u{200FF}',
        // Tags (plane 14)
        '\u{E0001}'..='\u{E007F}',
        // ASCII baseline for mix
        '\u{0041}'..='\u{005A}',
        // Combining marks
        '\u{0300}'..='\u{036F}',
    ]));
    prop::collection::vec(ranges, 1..32).prop_map(|chars| chars.into_iter().collect::<String>())
}

// ===========================================================================
// 1. NFKC implies NFC
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// If a string is in NFKC form, it must also be in NFC form.
    /// NFKC is stricter than NFC: it additionally normalizes compatibility chars.
    #[test]
    fn nfkc_implies_nfc(s in unicode_string_strategy()) {
        if s.is_nfkc() {
            prop_assert!(
                s.is_nfc(),
                "is_nfkc(s) is true but is_nfc(s) is false for {:?}",
                s
            );
        }
    }

    /// NFC applied to an already-NFKC string must be a no-op.
    #[test]
    fn nfc_of_nfkc_is_nfkc(s in unicode_string_strategy()) {
        let nfkc_s = s.nfkc();
        let nfc_of_nfkc = nfkc_s.nfc();
        prop_assert_eq!(
            &*nfc_of_nfkc, &*nfkc_s,
            "nfc(nfkc(s)) != nfkc(s)"
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// Same test with compatibility-heavy inputs.
    #[test]
    fn nfkc_implies_nfc_compat_heavy(s in compat_heavy_strategy()) {
        if s.is_nfkc() {
            prop_assert!(
                s.is_nfc(),
                "is_nfkc(s) is true but is_nfc(s) is false for {:?}",
                s
            );
        }
    }

    #[test]
    fn nfc_of_nfkc_is_nfkc_compat_heavy(s in compat_heavy_strategy()) {
        let nfkc_s = s.nfkc();
        let nfc_of_nfkc = nfkc_s.nfc();
        prop_assert_eq!(
            &*nfc_of_nfkc, &*nfkc_s,
            "nfc(nfkc(s)) != nfkc(s) for compat-heavy input"
        );
    }
}

// ===========================================================================
// 2. NFKD implies NFD
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// If a string is in NFKD form, it must also be in NFD form.
    #[test]
    fn nfkd_implies_nfd(s in unicode_string_strategy()) {
        if s.is_nfkd() {
            prop_assert!(
                s.is_nfd(),
                "is_nfkd(s) is true but is_nfd(s) is false for {:?}",
                s
            );
        }
    }

    /// NFD applied to an already-NFKD string must be a no-op.
    #[test]
    fn nfd_of_nfkd_is_nfkd(s in unicode_string_strategy()) {
        let nfkd_s = s.nfkd();
        let nfd_of_nfkd = nfkd_s.nfd();
        prop_assert_eq!(
            &*nfd_of_nfkd, &*nfkd_s,
            "nfd(nfkd(s)) != nfkd(s)"
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn nfkd_implies_nfd_compat_heavy(s in compat_heavy_strategy()) {
        if s.is_nfkd() {
            prop_assert!(
                s.is_nfd(),
                "is_nfkd(s) is true but is_nfd(s) is false for {:?}",
                s
            );
        }
    }

    #[test]
    fn nfd_of_nfkd_is_nfkd_compat_heavy(s in compat_heavy_strategy()) {
        let nfkd_s = s.nfkd();
        let nfd_of_nfkd = nfkd_s.nfd();
        prop_assert_eq!(
            &*nfd_of_nfkd, &*nfkd_s,
            "nfd(nfkd(s)) != nfkd(s) for compat-heavy input"
        );
    }
}

// ===========================================================================
// 3. NFKC(x) == NFC(NFKD(x)) -- fundamental Unicode invariant
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// The NFKC form can be computed by first applying NFKD then NFC.
    /// This is a core Unicode normalization invariant (UAX #15).
    #[test]
    fn nfkc_equals_nfc_of_nfkd(s in unicode_string_strategy()) {
        let nfkc_s = s.nfkc();
        let nfkd_s = s.nfkd();
        let nfc_of_nfkd = nfkd_s.nfc();
        prop_assert_eq!(
            &*nfkc_s, &*nfc_of_nfkd,
            "NFKC(s) != NFC(NFKD(s))"
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn nfkc_equals_nfc_of_nfkd_compat_heavy(s in compat_heavy_strategy()) {
        let nfkc_s = s.nfkc();
        let nfkd_s = s.nfkd();
        let nfc_of_nfkd = nfkd_s.nfc();
        prop_assert_eq!(
            &*nfkc_s, &*nfc_of_nfkd,
            "NFKC(s) != NFC(NFKD(s)) for compat-heavy input"
        );
    }
}

// ===========================================================================
// 4. NFKD(x) == NFD(NFKC(x)) -- dual invariant
// ===========================================================================
//
// Despite the task description suggesting this should NOT hold, it is in fact
// a valid Unicode invariant. Proof sketch:
//   NFKD(x) is the fully compatibility-decomposed, CCC-sorted form.
//   NFD(NFKC(x)) = NFD(NFC(NFKD(x)))  [by invariant #3]
//                 = NFD(NFKD(x))         [since NFD(NFC(y)) == NFD(y)]
//                 = NFKD(x)              [since NFKD(x) is already canonical-decomposed]
//
// We verify this invariant holds for all generated inputs.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn nfkd_equals_nfd_of_nfkc(s in unicode_string_strategy()) {
        let nfkd_s = s.nfkd();
        let nfkc_s = s.nfkc();
        let nfd_of_nfkc = nfkc_s.nfd();
        prop_assert_eq!(
            &*nfkd_s, &*nfd_of_nfkc,
            "NFKD(s) != NFD(NFKC(s))"
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn nfkd_equals_nfd_of_nfkc_compat_heavy(s in compat_heavy_strategy()) {
        let nfkd_s = s.nfkd();
        let nfkc_s = s.nfkc();
        let nfd_of_nfkc = nfkc_s.nfd();
        prop_assert_eq!(
            &*nfkd_s, &*nfd_of_nfkc,
            "NFKD(s) != NFD(NFKC(s)) for compat-heavy input"
        );
    }
}

// Deterministic tests showing that NFKD(x) != NFD(x) for compatibility chars,
// while NFKD(x) == NFD(NFKC(x)) still holds.
#[test]
fn nfkd_differs_from_nfd_but_equals_nfd_of_nfkc() {
    let inputs = [
        "\u{FB01}", // fi ligature
        "\u{00A0}", // NBSP
        "\u{FF21}", // fullwidth A
        "\u{2126}", // Ohm sign
        "\u{2460}", // circled digit one
        "\u{3300}", // CJK compat: square apaato
        "\u{FB20}", // Hebrew alternative ayin
        "\u{2075}", // superscript 5
        "\u{00BC}", // fraction one-quarter
        "\u{FB49}", // Hebrew shin with dagesh
    ];

    for input in &inputs {
        let nfkd = simd_normalizer::nfkd().normalize(input);
        let nfd = simd_normalizer::nfd().normalize(input);
        let nfkc = simd_normalizer::nfkc().normalize(input);
        let nfd_of_nfkc = simd_normalizer::nfd().normalize(&nfkc);

        // NFKD and NFD may differ for compatibility characters
        // (but they are the same for characters with only canonical decompositions).

        // The key invariant: NFKD(x) == NFD(NFKC(x)) always holds.
        assert_eq!(
            &*nfkd, &*nfd_of_nfkc,
            "NFKD({:?}) != NFD(NFKC({:?})): nfkd={:?}, nfd_of_nfkc={:?}",
            input, input, nfkd, nfd_of_nfkc
        );

        // Demonstrate that NFKD != NFD for chars with compatibility mappings
        // (fi ligature, NBSP, fullwidth, etc.)
        let has_compat_mapping = [
            "\u{FB01}", "\u{00A0}", "\u{FF21}", "\u{2460}", "\u{3300}", "\u{FB20}", "\u{2075}",
            "\u{00BC}",
        ];
        if has_compat_mapping.contains(input) {
            assert_ne!(
                &*nfkd, &*nfd,
                "Expected NFKD({:?}) != NFD({:?}) for compatibility char",
                input, input
            );
        }
    }
}

// ===========================================================================
// 3b. NFC(x) == NFC(NFD(x)) and NFKC(x) == NFKC(NFKD(x))
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// NFC is stable after NFD: normalizing to NFC gives the same result
    /// whether or not you decompose first.
    #[test]
    fn nfc_stable_after_nfd(s in unicode_string_strategy()) {
        let nfc_s = s.nfc();
        let nfd_s = s.nfd();
        let nfc_of_nfd = nfd_s.nfc();
        prop_assert_eq!(
            &*nfc_s, &*nfc_of_nfd,
            "NFC(s) != NFC(NFD(s))"
        );
    }

    /// NFKC is stable after NFKD.
    #[test]
    fn nfkc_stable_after_nfkd(s in unicode_string_strategy()) {
        let nfkc_s = s.nfkc();
        let nfkd_s = s.nfkd();
        let nfkc_of_nfkd = nfkd_s.nfkc();
        prop_assert_eq!(
            &*nfkc_s, &*nfkc_of_nfkd,
            "NFKC(s) != NFKC(NFKD(s))"
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn nfc_stable_after_nfd_compat_heavy(s in compat_heavy_strategy()) {
        let nfc_s = s.nfc();
        let nfd_s = s.nfd();
        let nfc_of_nfd = nfd_s.nfc();
        prop_assert_eq!(
            &*nfc_s, &*nfc_of_nfd,
            "NFC(s) != NFC(NFD(s)) for compat-heavy input"
        );
    }

    #[test]
    fn nfkc_stable_after_nfkd_compat_heavy(s in compat_heavy_strategy()) {
        let nfkc_s = s.nfkc();
        let nfkd_s = s.nfkd();
        let nfkc_of_nfkd = nfkd_s.nfkc();
        prop_assert_eq!(
            &*nfkc_s, &*nfkc_of_nfkd,
            "NFKC(s) != NFKC(NFKD(s)) for compat-heavy input"
        );
    }
}

// ===========================================================================
// 5. quick_check / normalize agreement
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// If quick_check returns Yes, normalize must return Cow::Borrowed.
    /// If quick_check returns No, normalize must return Cow::Owned (changed).
    #[test]
    fn quick_check_normalize_agreement_nfc(s in unicode_string_strategy()) {
        let norm = simd_normalizer::nfc();
        let qc = norm.quick_check(&s);
        let result = norm.normalize(&s);
        match qc {
            IsNormalized::Yes => {
                prop_assert!(
                    matches!(&result, Cow::Borrowed(_)),
                    "NFC quick_check=Yes but normalize returned Owned for {:?}",
                    s
                );
            }
            IsNormalized::No => {
                prop_assert!(
                    matches!(&result, Cow::Owned(_)),
                    "NFC quick_check=No but normalize returned Borrowed for {:?}",
                    s
                );
                prop_assert_ne!(
                    &*result, s.as_str(),
                    "NFC quick_check=No but normalize output equals input for {:?}",
                    s
                );
            }
            IsNormalized::Maybe => {
                // Maybe can go either way; just verify consistency.
            }
        }
    }

    #[test]
    fn quick_check_normalize_agreement_nfd(s in unicode_string_strategy()) {
        let norm = simd_normalizer::nfd();
        let qc = norm.quick_check(&s);
        let result = norm.normalize(&s);
        match qc {
            IsNormalized::Yes => {
                prop_assert!(
                    matches!(&result, Cow::Borrowed(_)),
                    "NFD quick_check=Yes but normalize returned Owned for {:?}",
                    s
                );
            }
            IsNormalized::No => {
                prop_assert!(
                    matches!(&result, Cow::Owned(_)),
                    "NFD quick_check=No but normalize returned Borrowed for {:?}",
                    s
                );
                prop_assert_ne!(
                    &*result, s.as_str(),
                    "NFD quick_check=No but normalize output equals input for {:?}",
                    s
                );
            }
            IsNormalized::Maybe => {}
        }
    }

    #[test]
    fn quick_check_normalize_agreement_nfkc(s in unicode_string_strategy()) {
        let norm = simd_normalizer::nfkc();
        let qc = norm.quick_check(&s);
        let result = norm.normalize(&s);
        match qc {
            IsNormalized::Yes => {
                prop_assert!(
                    matches!(&result, Cow::Borrowed(_)),
                    "NFKC quick_check=Yes but normalize returned Owned for {:?}",
                    s
                );
            }
            IsNormalized::No => {
                prop_assert!(
                    matches!(&result, Cow::Owned(_)),
                    "NFKC quick_check=No but normalize returned Borrowed for {:?}",
                    s
                );
                prop_assert_ne!(
                    &*result, s.as_str(),
                    "NFKC quick_check=No but normalize output equals input for {:?}",
                    s
                );
            }
            IsNormalized::Maybe => {}
        }
    }

    #[test]
    fn quick_check_normalize_agreement_nfkd(s in unicode_string_strategy()) {
        let norm = simd_normalizer::nfkd();
        let qc = norm.quick_check(&s);
        let result = norm.normalize(&s);
        match qc {
            IsNormalized::Yes => {
                prop_assert!(
                    matches!(&result, Cow::Borrowed(_)),
                    "NFKD quick_check=Yes but normalize returned Owned for {:?}",
                    s
                );
            }
            IsNormalized::No => {
                prop_assert!(
                    matches!(&result, Cow::Owned(_)),
                    "NFKD quick_check=No but normalize returned Borrowed for {:?}",
                    s
                );
                prop_assert_ne!(
                    &*result, s.as_str(),
                    "NFKD quick_check=No but normalize output equals input for {:?}",
                    s
                );
            }
            IsNormalized::Maybe => {}
        }
    }
}

// ===========================================================================
// 6. normalize_to matches normalize
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn normalize_to_matches_normalize_nfc(s in unicode_string_strategy()) {
        let norm = simd_normalizer::nfc();
        let expected = norm.normalize(&s);
        let mut buf = String::new();
        let was_normalized = norm.normalize_to(&s, &mut buf);
        prop_assert_eq!(
            &buf, &*expected,
            "NFC normalize_to output mismatch"
        );
        prop_assert_eq!(
            was_normalized,
            matches!(&expected, Cow::Borrowed(_)),
            "NFC normalize_to return value mismatch"
        );
    }

    #[test]
    fn normalize_to_matches_normalize_nfd(s in unicode_string_strategy()) {
        let norm = simd_normalizer::nfd();
        let expected = norm.normalize(&s);
        let mut buf = String::new();
        let was_normalized = norm.normalize_to(&s, &mut buf);
        prop_assert_eq!(
            &buf, &*expected,
            "NFD normalize_to output mismatch"
        );
        prop_assert_eq!(
            was_normalized,
            matches!(&expected, Cow::Borrowed(_)),
            "NFD normalize_to return value mismatch"
        );
    }

    #[test]
    fn normalize_to_matches_normalize_nfkc(s in unicode_string_strategy()) {
        let norm = simd_normalizer::nfkc();
        let expected = norm.normalize(&s);
        let mut buf = String::new();
        let was_normalized = norm.normalize_to(&s, &mut buf);
        prop_assert_eq!(
            &buf, &*expected,
            "NFKC normalize_to output mismatch"
        );
        prop_assert_eq!(
            was_normalized,
            matches!(&expected, Cow::Borrowed(_)),
            "NFKC normalize_to return value mismatch"
        );
    }

    #[test]
    fn normalize_to_matches_normalize_nfkd(s in unicode_string_strategy()) {
        let norm = simd_normalizer::nfkd();
        let expected = norm.normalize(&s);
        let mut buf = String::new();
        let was_normalized = norm.normalize_to(&s, &mut buf);
        prop_assert_eq!(
            &buf, &*expected,
            "NFKD normalize_to output mismatch"
        );
        prop_assert_eq!(
            was_normalized,
            matches!(&expected, Cow::Borrowed(_)),
            "NFKD normalize_to return value mismatch"
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn normalize_to_matches_normalize_supplementary_nfc(s in supplementary_heavy_strategy()) {
        let norm = simd_normalizer::nfc();
        let expected = norm.normalize(&s);
        let mut buf = String::new();
        let was_normalized = norm.normalize_to(&s, &mut buf);
        prop_assert_eq!(&buf, &*expected, "NFC normalize_to mismatch for supplementary input");
        prop_assert_eq!(was_normalized, matches!(&expected, Cow::Borrowed(_)));
    }

    #[test]
    fn normalize_to_matches_normalize_supplementary_nfd(s in supplementary_heavy_strategy()) {
        let norm = simd_normalizer::nfd();
        let expected = norm.normalize(&s);
        let mut buf = String::new();
        let was_normalized = norm.normalize_to(&s, &mut buf);
        prop_assert_eq!(&buf, &*expected, "NFD normalize_to mismatch for supplementary input");
        prop_assert_eq!(was_normalized, matches!(&expected, Cow::Borrowed(_)));
    }

    #[test]
    fn normalize_to_matches_normalize_supplementary_nfkc(s in supplementary_heavy_strategy()) {
        let norm = simd_normalizer::nfkc();
        let expected = norm.normalize(&s);
        let mut buf = String::new();
        let was_normalized = norm.normalize_to(&s, &mut buf);
        prop_assert_eq!(&buf, &*expected, "NFKC normalize_to mismatch for supplementary input");
        prop_assert_eq!(was_normalized, matches!(&expected, Cow::Borrowed(_)));
    }

    #[test]
    fn normalize_to_matches_normalize_supplementary_nfkd(s in supplementary_heavy_strategy()) {
        let norm = simd_normalizer::nfkd();
        let expected = norm.normalize(&s);
        let mut buf = String::new();
        let was_normalized = norm.normalize_to(&s, &mut buf);
        prop_assert_eq!(&buf, &*expected, "NFKD normalize_to mismatch for supplementary input");
        prop_assert_eq!(was_normalized, matches!(&expected, Cow::Borrowed(_)));
    }
}

/// normalize_to must correctly append to a non-empty buffer, matching
/// the normalize output.
#[test]
fn normalize_to_appends_correctly_all_forms() {
    let inputs = [
        "hello",
        "\u{00C5}",         // precomposed A-ring
        "e\u{0301}",        // e + combining acute
        "\u{1100}\u{1161}", // Hangul L+V
        "\u{AC00}",         // Hangul syllable
        "\u{FB01}",         // fi ligature
        "\u{2126}",         // Ohm sign
        "\u{00A0}",         // NBSP
    ];

    type NormFn = fn() -> Box<dyn Fn(&str) -> Cow<'_, str> + 'static>;
    type NormToFn = fn() -> Box<dyn Fn(&str, &mut String) -> bool + 'static>;
    let constructors: [(&str, NormFn, NormToFn); 4] = [
        (
            "NFC",
            || Box::new(|s: &str| simd_normalizer::nfc().normalize(s)),
            || Box::new(|s: &str, buf: &mut String| simd_normalizer::nfc().normalize_to(s, buf)),
        ),
        (
            "NFD",
            || Box::new(|s: &str| simd_normalizer::nfd().normalize(s)),
            || Box::new(|s: &str, buf: &mut String| simd_normalizer::nfd().normalize_to(s, buf)),
        ),
        (
            "NFKC",
            || Box::new(|s: &str| simd_normalizer::nfkc().normalize(s)),
            || Box::new(|s: &str, buf: &mut String| simd_normalizer::nfkc().normalize_to(s, buf)),
        ),
        (
            "NFKD",
            || Box::new(|s: &str| simd_normalizer::nfkd().normalize(s)),
            || Box::new(|s: &str, buf: &mut String| simd_normalizer::nfkd().normalize_to(s, buf)),
        ),
    ];

    for (label, make_norm, make_norm_to) in &constructors {
        let norm = make_norm();
        let norm_to = make_norm_to();
        for input in &inputs {
            let expected = norm(input);
            let mut buf = String::from("PREFIX:");
            norm_to(input, &mut buf);
            let expected_with_prefix = format!("PREFIX:{}", &*expected);
            assert_eq!(
                buf, expected_with_prefix,
                "{}: normalize_to did not append correctly for {:?}",
                label, input
            );
        }
    }
}

// ===========================================================================
// 7. is_normalized cross-form consistency
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// is_normalized must agree with normalize(s) == s for all 4 forms.
    /// This is tested within each form in properties.rs, but here we exercise
    /// cross-form edge cases by testing all 4 forms on each generated input.
    #[test]
    fn is_normalized_agrees_with_normalize_all_forms(s in unicode_string_strategy()) {
        // NFC
        let nfc_s = s.nfc();
        if &*nfc_s == s.as_str() {
            prop_assert!(s.is_nfc(), "nfc(s) == s but is_nfc is false for {:?}", s);
        }
        // NFD
        let nfd_s = s.nfd();
        if &*nfd_s == s.as_str() {
            prop_assert!(s.is_nfd(), "nfd(s) == s but is_nfd is false for {:?}", s);
        }
        // NFKC
        let nfkc_s = s.nfkc();
        if &*nfkc_s == s.as_str() {
            prop_assert!(s.is_nfkc(), "nfkc(s) == s but is_nfkc is false for {:?}", s);
        }
        // NFKD
        let nfkd_s = s.nfkd();
        if &*nfkd_s == s.as_str() {
            prop_assert!(s.is_nfkd(), "nfkd(s) == s but is_nfkd is false for {:?}", s);
        }
    }

    /// If is_nfkc is true then is_nfc must be true (subsumption).
    /// If is_nfkd is true then is_nfd must be true (subsumption).
    #[test]
    fn is_normalized_subsumption(s in unicode_string_strategy()) {
        if s.is_nfkc() {
            prop_assert!(s.is_nfc(), "is_nfkc but not is_nfc for {:?}", s);
        }
        if s.is_nfkd() {
            prop_assert!(s.is_nfd(), "is_nfkd but not is_nfd for {:?}", s);
        }
    }

    /// After normalization, is_normalized MUST return true for the result.
    #[test]
    fn normalize_output_is_normalized(s in unicode_string_strategy()) {
        let nfc_s = s.nfc();
        prop_assert!(nfc_s.is_nfc(), "nfc(s) result not recognized as NFC for {:?}", s);

        let nfd_s = s.nfd();
        prop_assert!(nfd_s.is_nfd(), "nfd(s) result not recognized as NFD for {:?}", s);

        let nfkc_s = s.nfkc();
        prop_assert!(nfkc_s.is_nfkc(), "nfkc(s) result not recognized as NFKC for {:?}", s);

        let nfkd_s = s.nfkd();
        prop_assert!(nfkd_s.is_nfkd(), "nfkd(s) result not recognized as NFKD for {:?}", s);
    }
}

// Cross-form is_normalized consistency for known edge-case strings.
#[test]
fn is_normalized_cross_form_deterministic() {
    // ASCII: all forms
    assert!("hello".is_nfc());
    assert!("hello".is_nfd());
    assert!("hello".is_nfkc());
    assert!("hello".is_nfkd());

    // Precomposed: NFC and NFKC yes, NFD and NFKD no
    assert!("\u{00C5}".is_nfc());
    assert!(!"\u{00C5}".is_nfd());
    assert!("\u{00C5}".is_nfkc());
    assert!(!"\u{00C5}".is_nfkd());

    // Decomposed: NFD and NFKD yes, NFC maybe/no
    let decomposed = "A\u{030A}";
    assert!(!decomposed.is_nfc());
    assert!(decomposed.is_nfd());
    assert!(!decomposed.is_nfkc());
    assert!(decomposed.is_nfkd());

    // Compatibility ligature: NFC and NFD yes (no canonical decomp),
    // NFKC and NFKD no
    assert!("\u{FB01}".is_nfc());
    assert!("\u{FB01}".is_nfd());
    assert!(!"\u{FB01}".is_nfkc());
    assert!(!"\u{FB01}".is_nfkd());

    // Hangul syllable: NFC yes, NFD no, NFKC yes, NFKD no
    assert!("\u{AC00}".is_nfc());
    assert!(!"\u{AC00}".is_nfd());
    assert!("\u{AC00}".is_nfkc());
    assert!(!"\u{AC00}".is_nfkd());

    // Empty string: all forms
    assert!("".is_nfc());
    assert!("".is_nfd());
    assert!("".is_nfkc());
    assert!("".is_nfkd());
}

// ===========================================================================
// 8. Decomposition subsumption
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// NFD(NFC(s)) == NFD(s): canonical decomposition erases composition differences.
    #[test]
    fn nfd_of_nfc_equals_nfd(s in unicode_string_strategy()) {
        let nfc_s = s.nfc();
        let nfd_of_nfc = nfc_s.nfd();
        let nfd_s = s.nfd();
        prop_assert_eq!(
            &*nfd_of_nfc, &*nfd_s,
            "NFD(NFC(s)) != NFD(s)"
        );
    }

    /// NFKD(NFKC(s)) == NFKD(s): compatibility decomposition erases composition differences.
    #[test]
    fn nfkd_of_nfkc_equals_nfkd(s in unicode_string_strategy()) {
        let nfkc_s = s.nfkc();
        let nfkd_of_nfkc = nfkc_s.nfkd();
        let nfkd_s = s.nfkd();
        prop_assert_eq!(
            &*nfkd_of_nfkc, &*nfkd_s,
            "NFKD(NFKC(s)) != NFKD(s)"
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn nfd_of_nfc_equals_nfd_compat_heavy(s in compat_heavy_strategy()) {
        let nfc_s = s.nfc();
        let nfd_of_nfc = nfc_s.nfd();
        let nfd_s = s.nfd();
        prop_assert_eq!(
            &*nfd_of_nfc, &*nfd_s,
            "NFD(NFC(s)) != NFD(s) for compat-heavy input"
        );
    }

    #[test]
    fn nfkd_of_nfkc_equals_nfkd_compat_heavy(s in compat_heavy_strategy()) {
        let nfkc_s = s.nfkc();
        let nfkd_of_nfkc = nfkc_s.nfkd();
        let nfkd_s = s.nfkd();
        prop_assert_eq!(
            &*nfkd_of_nfkc, &*nfkd_s,
            "NFKD(NFKC(s)) != NFKD(s) for compat-heavy input"
        );
    }
}

// ===========================================================================
// 9. Cross-validation with ICU4X
// ===========================================================================

/// Verify that all 4 normalization forms match ICU4X output for a set of
/// tricky inputs covering edge cases.
#[test]
fn cross_validate_icu4x_tricky_inputs() {
    let inputs = [
        // Hangul
        "\u{AC00}",                 // Hangul syllable GA
        "\u{1100}\u{1161}",         // Hangul L+V (composes to GA)
        "\u{1100}\u{1161}\u{11A8}", // Hangul L+V+T
        "\u{D4DB}",                 // Last Hangul syllable with T
        // Composition exclusions
        "\u{2126}", // Ohm sign
        "\u{212A}", // Kelvin sign
        "\u{212B}", // Angstrom sign
        "\u{0958}", // Devanagari QA
        "\u{FB1D}", // Hebrew yod with hiriq
        "\u{FB2A}", // Hebrew shin with shin dot
        // Combining mark singletons
        "\u{0340}", // Combining grave tone mark
        "\u{0341}", // Combining acute tone mark
        "\u{0344}", // Combining Greek dialytika tonos
        // Compatibility characters
        "\u{FB01}", // fi ligature
        "\u{00A0}", // NBSP
        "\u{FF21}", // Fullwidth A
        "\u{2075}", // Superscript 5
        "\u{00BC}", // Fraction one-quarter
        "\u{FB49}", // Hebrew shin with dagesh
        // Multi-character combining sequences
        "a\u{0308}\u{0301}", // a + diaeresis + acute
        "A\u{0327}\u{030A}", // A + cedilla + ring above
        "\u{1E0A}\u{0323}",  // D-dot-above + dot-below
        // CJK compatibility ideograph
        "\u{2F800}", // CJK compat ideograph
        // Zero-width chars blocking composition
        "e\u{200C}\u{0301}", // ZWNJ blocks composition
        "e\u{034F}\u{0301}", // CGJ blocks composition
        // BOM + combining
        "\u{FEFF}a\u{0308}",
        // Mixed scripts
        "Hello\u{0300}World\u{0301}\u{AC00}\u{FB01}",
        // Empty string
        "",
        // Pure ASCII
        "The quick brown fox jumps over the lazy dog.",
    ];

    let icu_nfc = ComposingNormalizerBorrowed::new_nfc();
    let icu_nfd = DecomposingNormalizerBorrowed::new_nfd();
    let icu_nfkc = ComposingNormalizerBorrowed::new_nfkc();
    let icu_nfkd = DecomposingNormalizerBorrowed::new_nfkd();

    for input in &inputs {
        // NFC
        let simd_nfc = simd_normalizer::nfc().normalize(input);
        let icu_nfc_result = icu_nfc.normalize(input);
        assert_eq!(
            &*simd_nfc, &*icu_nfc_result,
            "NFC mismatch with ICU4X for {:?}: simd={:?}, icu={:?}",
            input, simd_nfc, icu_nfc_result
        );

        // NFD
        let simd_nfd = simd_normalizer::nfd().normalize(input);
        let icu_nfd_result = icu_nfd.normalize(input);
        assert_eq!(
            &*simd_nfd, &*icu_nfd_result,
            "NFD mismatch with ICU4X for {:?}: simd={:?}, icu={:?}",
            input, simd_nfd, icu_nfd_result
        );

        // NFKC
        let simd_nfkc = simd_normalizer::nfkc().normalize(input);
        let icu_nfkc_result = icu_nfkc.normalize(input);
        assert_eq!(
            &*simd_nfkc, &*icu_nfkc_result,
            "NFKC mismatch with ICU4X for {:?}: simd={:?}, icu={:?}",
            input, simd_nfkc, icu_nfkc_result
        );

        // NFKD
        let simd_nfkd = simd_normalizer::nfkd().normalize(input);
        let icu_nfkd_result = icu_nfkd.normalize(input);
        assert_eq!(
            &*simd_nfkd, &*icu_nfkd_result,
            "NFKD mismatch with ICU4X for {:?}: simd={:?}, icu={:?}",
            input, simd_nfkd, icu_nfkd_result
        );

        // Also verify is_normalized agrees with ICU4X
        assert_eq!(
            input.is_nfc(),
            icu_nfc.is_normalized(input),
            "is_nfc mismatch with ICU4X for {:?}",
            input
        );
        assert_eq!(
            input.is_nfd(),
            icu_nfd.is_normalized(input),
            "is_nfd mismatch with ICU4X for {:?}",
            input
        );
        assert_eq!(
            input.is_nfkc(),
            icu_nfkc.is_normalized(input),
            "is_nfkc mismatch with ICU4X for {:?}",
            input
        );
        assert_eq!(
            input.is_nfkd(),
            icu_nfkd.is_normalized(input),
            "is_nfkd mismatch with ICU4X for {:?}",
            input
        );
    }
}

// Property-based ICU4X cross-validation for all 4 forms.
proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn cross_validate_icu4x_nfc(s in unicode_string_strategy()) {
        let simd_result = simd_normalizer::nfc().normalize(&s);
        let icu_result = ComposingNormalizerBorrowed::new_nfc().normalize(&s);
        prop_assert_eq!(
            &*simd_result, &*icu_result,
            "NFC cross-validation failed with ICU4X"
        );
    }

    #[test]
    fn cross_validate_icu4x_nfd(s in unicode_string_strategy()) {
        let simd_result = simd_normalizer::nfd().normalize(&s);
        let icu_result = DecomposingNormalizerBorrowed::new_nfd().normalize(&s);
        prop_assert_eq!(
            &*simd_result, &*icu_result,
            "NFD cross-validation failed with ICU4X"
        );
    }

    #[test]
    fn cross_validate_icu4x_nfkc(s in unicode_string_strategy()) {
        let simd_result = simd_normalizer::nfkc().normalize(&s);
        let icu_result = ComposingNormalizerBorrowed::new_nfkc().normalize(&s);
        prop_assert_eq!(
            &*simd_result, &*icu_result,
            "NFKC cross-validation failed with ICU4X"
        );
    }

    #[test]
    fn cross_validate_icu4x_nfkd(s in unicode_string_strategy()) {
        let simd_result = simd_normalizer::nfkd().normalize(&s);
        let icu_result = DecomposingNormalizerBorrowed::new_nfkd().normalize(&s);
        prop_assert_eq!(
            &*simd_result, &*icu_result,
            "NFKD cross-validation failed with ICU4X"
        );
    }
}

// ===========================================================================
// Deterministic cross-form tests for edge cases
// ===========================================================================

/// Hangul: verify cross-form relationships on Hangul syllables and jamo.
#[test]
fn hangul_cross_form_invariants() {
    // Precomposed syllable GA (U+AC00)
    let ga = "\u{AC00}";
    let ga_jamo = "\u{1100}\u{1161}"; // L + V

    // NFC composes jamo to syllable
    assert_eq!(&*ga_jamo.nfc(), ga);
    // NFD decomposes syllable to jamo
    assert_eq!(&*ga.nfd(), ga_jamo);

    // NFKC and NFC agree for Hangul (no compatibility mapping)
    assert_eq!(&*ga.nfkc(), &*ga.nfc());
    assert_eq!(&*ga_jamo.nfkc(), &*ga_jamo.nfc());

    // NFKD and NFD agree for Hangul (no compatibility mapping)
    assert_eq!(&*ga.nfkd(), &*ga.nfd());
    assert_eq!(&*ga_jamo.nfkd(), &*ga_jamo.nfd());

    // Invariant 3: NFKC == NFC(NFKD)
    assert_eq!(&*ga.nfkc(), &*ga.nfkd().nfc());
    assert_eq!(&*ga_jamo.nfkc(), &*ga_jamo.nfkd().nfc());

    // Invariant 8: NFD(NFC) == NFD
    assert_eq!(&*ga.nfc().nfd(), &*ga.nfd());
    assert_eq!(&*ga_jamo.nfc().nfd(), &*ga_jamo.nfd());

    // Hangul with trailing consonant
    let lvt = "\u{1100}\u{1161}\u{11A8}"; // L + V + T
    let syllable = "\u{AC01}"; // Precomposed LVT syllable
    assert_eq!(&*lvt.nfc(), syllable);
    assert_eq!(&*syllable.nfd(), lvt);
    assert_eq!(&*syllable.nfkc(), &*syllable.nfkd().nfc());
    assert_eq!(&*syllable.nfc().nfd(), &*syllable.nfd());
}

/// Composition exclusions: verify cross-form behavior.
#[test]
fn composition_exclusions_cross_form() {
    // For characters with CANONICAL decompositions, NFC == NFKC and NFD == NFKD.
    let canonical_exclusions = [
        "\u{2126}", // Ohm sign
        "\u{212A}", // Kelvin sign
        "\u{212B}", // Angstrom sign
        "\u{0340}", // Combining grave tone mark
        "\u{0341}", // Combining acute tone mark
        "\u{0344}", // Combining Greek dialytika tonos
        "\u{0958}", // Devanagari QA
        "\u{FB1D}", // Hebrew yod with hiriq
    ];

    for input in &canonical_exclusions {
        assert_eq!(
            &*input.nfc(),
            &*input.nfkc(),
            "NFC != NFKC for canonical exclusion {:?}",
            input
        );
        assert_eq!(
            &*input.nfd(),
            &*input.nfkd(),
            "NFD != NFKD for canonical exclusion {:?}",
            input
        );

        // Invariant 3: NFKC == NFC(NFKD)
        assert_eq!(
            &*input.nfkc(),
            &*input.nfkd().nfc(),
            "NFKC != NFC(NFKD) for canonical exclusion {:?}",
            input
        );

        // Invariant 8: NFD(NFC) == NFD
        assert_eq!(
            &*input.nfc().nfd(),
            &*input.nfd(),
            "NFD(NFC) != NFD for canonical exclusion {:?}",
            input
        );
    }
}

/// Compatibility characters: NFC != NFKC, NFD != NFKD, but cross-form
/// invariants still hold.
#[test]
fn compatibility_chars_cross_form() {
    let compat_chars = [
        ("\u{FB01}", "fi"), // fi ligature -> "fi"
        ("\u{00A0}", " "),  // NBSP -> space
        ("\u{FF21}", "A"),  // Fullwidth A -> A
        ("\u{2075}", "5"),  // Superscript 5 -> 5
    ];

    for (input, expected_compat) in &compat_chars {
        // NFC/NFD leave compatibility chars unchanged
        assert_eq!(&*input.nfc(), *input, "NFC changed compat char {:?}", input);
        assert_eq!(&*input.nfd(), *input, "NFD changed compat char {:?}", input);

        // NFKC/NFKD decompose them
        assert_eq!(
            &*input.nfkc(),
            *expected_compat,
            "NFKC mismatch for {:?}",
            input
        );
        assert_eq!(
            &*input.nfkd(),
            *expected_compat,
            "NFKD mismatch for {:?}",
            input
        );

        // Invariant 3: NFKC == NFC(NFKD)
        assert_eq!(
            &*input.nfkc(),
            &*input.nfkd().nfc(),
            "NFKC != NFC(NFKD) for compat char {:?}",
            input
        );

        // Invariant 8: NFKD(NFKC) == NFKD
        assert_eq!(
            &*input.nfkc().nfkd(),
            &*input.nfkd(),
            "NFKD(NFKC) != NFKD for compat char {:?}",
            input
        );
    }
}

/// Multi-combining mark sequences: verify cross-form relationships hold
/// with non-trivial CCC reordering.
#[test]
fn multi_combining_cross_form() {
    // a + cedilla (CCC=202) + acute (CCC=230)
    let input = "a\u{0327}\u{0301}";

    // All 4 forms
    let nfc = input.nfc();
    let nfd = input.nfd();
    let nfkc = input.nfkc();
    let nfkd = input.nfkd();

    // For purely canonical sequences, NFC == NFKC and NFD == NFKD
    assert_eq!(
        &*nfc, &*nfkc,
        "NFC != NFKC for canonical combining sequence"
    );
    assert_eq!(
        &*nfd, &*nfkd,
        "NFD != NFKD for canonical combining sequence"
    );

    // Invariant 3: NFKC == NFC(NFKD)
    assert_eq!(&*nfkc, &*nfkd.nfc(), "NFKC != NFC(NFKD)");

    // Invariant 8: NFD(NFC) == NFD
    assert_eq!(&*nfc.nfd(), &*nfd, "NFD(NFC) != NFD");

    // Dot-above + dot-below (CCC reordering)
    let input2 = "\u{1E0A}\u{0323}"; // D-dot-above + dot-below
    let nfc2 = input2.nfc();
    let nfd2 = input2.nfd();

    // Invariant 8: NFD(NFC) == NFD
    assert_eq!(
        &*nfc2.nfd(),
        &*nfd2,
        "NFD(NFC) != NFD for dot-above+dot-below"
    );

    // Invariant 3
    assert_eq!(
        &*input2.nfkc(),
        &*input2.nfkd().nfc(),
        "NFKC != NFC(NFKD) for dot-above+dot-below"
    );
}

/// Quick-check deterministic tests for specific known inputs.
#[test]
fn quick_check_deterministic_agreement() {
    // Pure ASCII: QC=Yes for all forms, normalize returns Borrowed
    let ascii = "Hello, world!";

    // NFC
    assert_eq!(simd_normalizer::nfc().quick_check(ascii), IsNormalized::Yes);
    assert!(matches!(
        simd_normalizer::nfc().normalize(ascii),
        Cow::Borrowed(_)
    ));

    // NFD
    assert_eq!(simd_normalizer::nfd().quick_check(ascii), IsNormalized::Yes);
    assert!(matches!(
        simd_normalizer::nfd().normalize(ascii),
        Cow::Borrowed(_)
    ));

    // NFKC
    assert_eq!(
        simd_normalizer::nfkc().quick_check(ascii),
        IsNormalized::Yes
    );
    assert!(matches!(
        simd_normalizer::nfkc().normalize(ascii),
        Cow::Borrowed(_)
    ));

    // NFKD
    assert_eq!(
        simd_normalizer::nfkd().quick_check(ascii),
        IsNormalized::Yes
    );
    assert!(matches!(
        simd_normalizer::nfkd().normalize(ascii),
        Cow::Borrowed(_)
    ));

    // Precomposed char: NFC=Yes, NFD=No
    let precomposed = "\u{00C5}";
    assert_eq!(
        simd_normalizer::nfc().quick_check(precomposed),
        IsNormalized::Yes
    );
    assert_eq!(
        simd_normalizer::nfd().quick_check(precomposed),
        IsNormalized::No
    );
    assert!(matches!(
        simd_normalizer::nfd().normalize(precomposed),
        Cow::Owned(_)
    ));

    // fi ligature: NFC=Yes, NFKC=No
    let fi = "\u{FB01}";
    assert_eq!(simd_normalizer::nfc().quick_check(fi), IsNormalized::Yes);
    assert_eq!(simd_normalizer::nfkc().quick_check(fi), IsNormalized::No);
    assert!(matches!(
        simd_normalizer::nfkc().normalize(fi),
        Cow::Owned(_)
    ));

    // Ohm sign: No for all forms
    let ohm = "\u{2126}";
    assert_eq!(simd_normalizer::nfc().quick_check(ohm), IsNormalized::No);
    assert_eq!(simd_normalizer::nfd().quick_check(ohm), IsNormalized::No);
    assert_eq!(simd_normalizer::nfkc().quick_check(ohm), IsNormalized::No);
    assert_eq!(simd_normalizer::nfkd().quick_check(ohm), IsNormalized::No);
}

/// Verify all cross-form invariants hold for a large deterministic set of
/// edge-case strings, cross-validated against ICU4X.
#[test]
fn comprehensive_deterministic_cross_form() {
    let icu_nfc = ComposingNormalizerBorrowed::new_nfc();
    let icu_nfd = DecomposingNormalizerBorrowed::new_nfd();
    let icu_nfkc = ComposingNormalizerBorrowed::new_nfkc();
    let icu_nfkd = DecomposingNormalizerBorrowed::new_nfkd();

    let inputs: Vec<&str> = vec![
        "",
        "ASCII only",
        "\u{00E9}",                 // precomposed e-acute
        "e\u{0301}",                // decomposed e-acute
        "\u{AC00}",                 // Hangul GA
        "\u{1100}\u{1161}",         // Hangul jamo L+V
        "\u{1100}\u{1161}\u{11A8}", // Hangul jamo L+V+T
        "\u{FB01}",                 // fi ligature
        "\u{00A0}",                 // NBSP
        "\u{2126}",                 // Ohm sign
        "\u{212A}",                 // Kelvin sign
        "\u{212B}",                 // Angstrom sign
        "\u{FF21}\u{FF22}\u{FF23}", // Fullwidth ABC
        "\u{0958}",                 // Devanagari QA
        "\u{0340}",                 // Combining grave tone mark
        "a\u{0308}\u{0301}",        // a + diaeresis + acute
        "A\u{0327}\u{030A}",        // A + cedilla + ring above
        "\u{1E0A}\u{0323}",         // D-dot-above + dot-below
        "\u{FEFF}a\u{0308}",        // BOM + a + diaeresis
        "\u{2F800}",                // CJK compat ideograph
        "\u{00BC}",                 // Fraction one-quarter
        "\u{2075}",                 // Superscript 5
        "e\u{200C}\u{0301}",        // ZWNJ blocking composition
        "\u{D4DB}",                 // Last Hangul with trailing
    ];

    for input in &inputs {
        let nfc = simd_normalizer::nfc().normalize(input);
        let nfd = simd_normalizer::nfd().normalize(input);
        let nfkc = simd_normalizer::nfkc().normalize(input);
        let nfkd = simd_normalizer::nfkd().normalize(input);

        // Invariant 3: NFKC == NFC(NFKD)
        let nfc_of_nfkd = simd_normalizer::nfc().normalize(&nfkd);
        assert_eq!(&*nfkc, &*nfc_of_nfkd, "NFKC != NFC(NFKD) for {:?}", input);

        // Invariant 4: NFKD == NFD(NFKC)
        let nfd_of_nfkc = simd_normalizer::nfd().normalize(&nfkc);
        assert_eq!(&*nfkd, &*nfd_of_nfkc, "NFKD != NFD(NFKC) for {:?}", input);

        // Invariant 8a: NFD(NFC) == NFD
        let nfd_of_nfc = simd_normalizer::nfd().normalize(&nfc);
        assert_eq!(&*nfd_of_nfc, &*nfd, "NFD(NFC) != NFD for {:?}", input);

        // Invariant 8b: NFKD(NFKC) == NFKD
        let nfkd_of_nfkc = simd_normalizer::nfkd().normalize(&nfkc);
        assert_eq!(&*nfkd_of_nfkc, &*nfkd, "NFKD(NFKC) != NFKD for {:?}", input);

        // Cross-validate all 4 forms with ICU4X
        assert_eq!(
            &*nfc,
            &*icu_nfc.normalize(input),
            "NFC ICU4X mismatch for {:?}",
            input
        );
        assert_eq!(
            &*nfd,
            &*icu_nfd.normalize(input),
            "NFD ICU4X mismatch for {:?}",
            input
        );
        assert_eq!(
            &*nfkc,
            &*icu_nfkc.normalize(input),
            "NFKC ICU4X mismatch for {:?}",
            input
        );
        assert_eq!(
            &*nfkd,
            &*icu_nfkd.normalize(input),
            "NFKD ICU4X mismatch for {:?}",
            input
        );
    }
}
