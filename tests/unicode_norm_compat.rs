// tests/unicode_norm_compat.rs
//! Compatibility tests ported from the `unicode-normalization` crate.
//!
//! These tests validate that `simd-normalizer` produces the same results as
//! `unicode-normalization` for specific test vectors, CJK compatibility
//! ideographs, quick-check cross-form consistency, and a full differential
//! pass over the official conformance data.
//!
//! This file does NOT duplicate:
//! - conformance.rs: UAX#15 20-invariant normalization tests
//! - exhaustive.rs: per-codepoint differential tests vs icu_normalizer
//! - properties.rs: proptest property-based tests

mod data;
use data::normalization_tests::NORMALIZATION_TESTS;
use simd_normalizer::UnicodeNormalization;

// ============================================================================
// Section 1: Specific test vectors from unicode-normalization src/test.rs
// ============================================================================

#[test]
fn test_nfd_vectors() {
    let cases: &[(&str, &str)] = &[
        ("abc", "abc"),
        ("\u{1e0b}\u{1c4}", "d\u{307}\u{1c4}"),
        ("\u{2026}", "\u{2026}"),
        ("\u{2126}", "\u{3a9}"),
        ("\u{1e0b}\u{323}", "d\u{323}\u{307}"),
        ("\u{1e0d}\u{307}", "d\u{323}\u{307}"),
        ("a\u{301}", "a\u{301}"),
        ("\u{301}a", "\u{301}a"),
        ("\u{d4db}", "\u{1111}\u{1171}\u{11b6}"),
        ("\u{ac1c}", "\u{1100}\u{1162}"),
    ];

    for (input, expected) in cases {
        let result = input.nfd();
        assert_eq!(
            &*result, *expected,
            "NFD({:?}): got {:?}, expected {:?}",
            input, &*result, expected
        );
    }
}

#[test]
fn test_nfkd_vectors() {
    let cases: &[(&str, &str)] = &[
        ("abc", "abc"),
        ("\u{1e0b}\u{1c4}", "d\u{307}DZ\u{30c}"),
        ("\u{2026}", "..."),
        ("\u{2126}", "\u{3a9}"),
        ("\u{1e0b}\u{323}", "d\u{323}\u{307}"),
        ("\u{1e0d}\u{307}", "d\u{323}\u{307}"),
        ("a\u{301}", "a\u{301}"),
        ("\u{301}a", "\u{301}a"),
        ("\u{d4db}", "\u{1111}\u{1171}\u{11b6}"),
        ("\u{ac1c}", "\u{1100}\u{1162}"),
    ];

    for (input, expected) in cases {
        let result = input.nfkd();
        assert_eq!(
            &*result, *expected,
            "NFKD({:?}): got {:?}, expected {:?}",
            input, &*result, expected
        );
    }
}

#[test]
fn test_nfc_vectors() {
    let cases: &[(&str, &str)] = &[
        ("abc", "abc"),
        ("\u{1e0b}\u{1c4}", "\u{1e0b}\u{1c4}"),
        ("\u{2026}", "\u{2026}"),
        ("\u{2126}", "\u{3a9}"),
        ("\u{1e0b}\u{323}", "\u{1e0d}\u{307}"),
        ("\u{1e0d}\u{307}", "\u{1e0d}\u{307}"),
        ("a\u{301}", "\u{e1}"),
        ("\u{301}a", "\u{301}a"),
        ("\u{d4db}", "\u{d4db}"),
        ("\u{ac1c}", "\u{ac1c}"),
        (
            "a\u{300}\u{305}\u{315}\u{5ae}b",
            "\u{e0}\u{5ae}\u{305}\u{315}b",
        ),
    ];

    for (input, expected) in cases {
        let result = input.nfc();
        assert_eq!(
            &*result, *expected,
            "NFC({:?}): got {:?}, expected {:?}",
            input, &*result, expected
        );
    }
}

#[test]
fn test_nfkc_vectors() {
    let cases: &[(&str, &str)] = &[
        ("abc", "abc"),
        ("\u{1e0b}\u{1c4}", "\u{1e0b}D\u{17d}"),
        ("\u{2026}", "..."),
        ("\u{2126}", "\u{3a9}"),
        ("\u{1e0b}\u{323}", "\u{1e0d}\u{307}"),
        ("\u{1e0d}\u{307}", "\u{1e0d}\u{307}"),
        ("a\u{301}", "\u{e1}"),
        ("\u{301}a", "\u{301}a"),
        ("\u{d4db}", "\u{d4db}"),
        ("\u{ac1c}", "\u{ac1c}"),
        (
            "a\u{300}\u{305}\u{315}\u{5ae}b",
            "\u{e0}\u{5ae}\u{305}\u{315}b",
        ),
    ];

    for (input, expected) in cases {
        let result = input.nfkc();
        assert_eq!(
            &*result, *expected,
            "NFKC({:?}): got {:?}, expected {:?}",
            input, &*result, expected
        );
    }
}

// ============================================================================
// Section 2: CJK Compatibility Ideograph Decomposition
// ============================================================================

#[test]
fn test_cjk_compat_decomposition() {
    // U+2F999 (CJK compat) -> U+831D (singleton canonical decomposition)
    // U+2F8A6 (CJK compat) -> U+6148 (singleton canonical decomposition)
    let s = "\u{2f999}\u{2f8a6}";
    let expected = "\u{831d}\u{6148}";

    assert_eq!(&*s.nfd(), expected, "NFD of CJK compat ideographs");
    assert_eq!(&*s.nfkd(), expected, "NFKD of CJK compat ideographs");
    assert_eq!(&*s.nfc(), expected, "NFC of CJK compat ideographs");
    assert_eq!(&*s.nfkc(), expected, "NFKC of CJK compat ideographs");
}

// ============================================================================
// Section 3: Quick-Check Cross-Form Consistency
//
// Ported from unicode-normalization tests/tests.rs test_quick_check.
// This tests relationships BETWEEN normalization forms that are not covered
// by conformance.rs (which only checks is_X(X_column)==true).
// ============================================================================

#[test]
fn test_quick_check_cross_form_consistency() {
    let mut failures = Vec::new();

    for (i, t) in NORMALIZATION_TESTS.iter().enumerate() {
        // Basic: normalized forms must pass their own quick-check.
        // (These are also in conformance.rs, but included here for completeness
        // of the cross-form logic below which depends on them.)
        if !t.nfc.is_nfc() {
            failures.push(format!(
                "case {i}: is_nfc(nfc) should be true, nfc={:?}",
                t.nfc
            ));
        }
        if !t.nfd.is_nfd() {
            failures.push(format!(
                "case {i}: is_nfd(nfd) should be true, nfd={:?}",
                t.nfd
            ));
        }
        if !t.nfkc.is_nfkc() {
            failures.push(format!(
                "case {i}: is_nfkc(nfkc) should be true, nfkc={:?}",
                t.nfkc
            ));
        }
        if !t.nfkd.is_nfkd() {
            failures.push(format!(
                "case {i}: is_nfkd(nfkd) should be true, nfkd={:?}",
                t.nfkd
            ));
        }

        // Cross-form: if NFC != NFD, then NFD should not be NFC and vice versa.
        if t.nfc != t.nfd {
            if t.nfd.is_nfc() {
                failures.push(format!(
                    "case {i}: nfc!=nfd but is_nfc(nfd) is true; nfc={:?} nfd={:?}",
                    t.nfc, t.nfd
                ));
            }
            if t.nfc.is_nfd() {
                failures.push(format!(
                    "case {i}: nfc!=nfd but is_nfd(nfc) is true; nfc={:?} nfd={:?}",
                    t.nfc, t.nfd
                ));
            }
        }

        // Cross-form: if NFKC != NFC, then NFC is not NFKC, but NFKC is NFC.
        if t.nfkc != t.nfc {
            if t.nfc.is_nfkc() {
                failures.push(format!(
                    "case {i}: nfkc!=nfc but is_nfkc(nfc) is true; nfkc={:?} nfc={:?}",
                    t.nfkc, t.nfc
                ));
            }
            if !t.nfkc.is_nfc() {
                failures.push(format!(
                    "case {i}: nfkc!=nfc but is_nfc(nfkc) is false; nfkc={:?} nfc={:?}",
                    t.nfkc, t.nfc
                ));
            }
        }

        // Cross-form: if NFKD != NFD, then NFD is not NFKD, but NFKD is NFD.
        if t.nfkd != t.nfd {
            if t.nfd.is_nfkd() {
                failures.push(format!(
                    "case {i}: nfkd!=nfd but is_nfkd(nfd) is true; nfkd={:?} nfd={:?}",
                    t.nfkd, t.nfd
                ));
            }
            if !t.nfkd.is_nfd() {
                failures.push(format!(
                    "case {i}: nfkd!=nfd but is_nfd(nfkd) is false; nfkd={:?} nfd={:?}",
                    t.nfkd, t.nfd
                ));
            }
        }

        // Bail early on too many failures to keep output readable.
        if failures.len() > 100 {
            failures.push("... (truncated after 100 failures)".to_string());
            break;
        }
    }

    assert!(
        failures.is_empty(),
        "Quick-check cross-form consistency failures ({} failures):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

// ============================================================================
// Section 4: Differential Validation Against unicode-normalization
//
// Cross-validates simd-normalizer against the unicode-normalization crate
// (a second reference implementation, separate from icu_normalizer used in
// exhaustive.rs). Uses the trait-based API with iterators from
// unicode-normalization.
// ============================================================================

/// Helper: normalize via unicode-normalization crate (iterator-based API).
mod un_reference {
    use unicode_normalization::UnicodeNormalization as UNUnicodeNormalization;

    pub fn nfc(s: &str) -> String {
        s.nfc().collect()
    }

    pub fn nfd(s: &str) -> String {
        s.nfd().collect()
    }

    pub fn nfkc(s: &str) -> String {
        s.nfkc().collect()
    }

    pub fn nfkd(s: &str) -> String {
        s.nfkd().collect()
    }
}

#[test]
fn test_differential_vs_unicode_normalization() {
    let mut failures = Vec::new();

    for (i, t) in NORMALIZATION_TESTS.iter().enumerate() {
        // Test all four forms on all five columns (source, nfc, nfd, nfkc, nfkd)
        let columns: &[(&str, &str)] = &[
            ("source", t.source),
            ("nfc", t.nfc),
            ("nfd", t.nfd),
            ("nfkc", t.nfkc),
            ("nfkd", t.nfkd),
        ];

        for &(col_name, input) in columns {
            // NFC
            let ours = input.nfc();
            let theirs = un_reference::nfc(input);
            if *ours != theirs {
                failures.push(format!(
                    "case {i} NFC({col_name}): simd={:?} un={:?} (input={:?})",
                    &*ours, theirs, input
                ));
            }

            // NFD
            let ours = input.nfd();
            let theirs = un_reference::nfd(input);
            if *ours != theirs {
                failures.push(format!(
                    "case {i} NFD({col_name}): simd={:?} un={:?} (input={:?})",
                    &*ours, theirs, input
                ));
            }

            // NFKC
            let ours = input.nfkc();
            let theirs = un_reference::nfkc(input);
            if *ours != theirs {
                failures.push(format!(
                    "case {i} NFKC({col_name}): simd={:?} un={:?} (input={:?})",
                    &*ours, theirs, input
                ));
            }

            // NFKD
            let ours = input.nfkd();
            let theirs = un_reference::nfkd(input);
            if *ours != theirs {
                failures.push(format!(
                    "case {i} NFKD({col_name}): simd={:?} un={:?} (input={:?})",
                    &*ours, theirs, input
                ));
            }
        }

        if failures.len() > 100 {
            failures.push("... (truncated after 100 failures)".to_string());
            break;
        }
    }

    assert!(
        failures.is_empty(),
        "Differential failures against unicode-normalization ({} failures):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// Also cross-validate the specific test vectors from Section 1 against
/// unicode-normalization, ensuring both libraries agree on these cases.
#[test]
fn test_specific_vectors_differential() {
    // All inputs used in the Section 1 vectors
    let inputs: &[&str] = &[
        "abc",
        "\u{1e0b}\u{1c4}",
        "\u{2026}",
        "\u{2126}",
        "\u{1e0b}\u{323}",
        "\u{1e0d}\u{307}",
        "a\u{301}",
        "\u{301}a",
        "\u{d4db}",
        "\u{ac1c}",
        "a\u{300}\u{305}\u{315}\u{5ae}b",
        "\u{2f999}\u{2f8a6}", // CJK compat
    ];

    for input in inputs {
        // NFC
        let ours = input.nfc();
        let theirs = un_reference::nfc(input);
        assert_eq!(
            &*ours, &*theirs,
            "NFC({:?}): simd={:?} un={:?}",
            input, &*ours, theirs
        );

        // NFD
        let ours = input.nfd();
        let theirs = un_reference::nfd(input);
        assert_eq!(
            &*ours, &*theirs,
            "NFD({:?}): simd={:?} un={:?}",
            input, &*ours, theirs
        );

        // NFKC
        let ours = input.nfkc();
        let theirs = un_reference::nfkc(input);
        assert_eq!(
            &*ours, &*theirs,
            "NFKC({:?}): simd={:?} un={:?}",
            input, &*ours, theirs
        );

        // NFKD
        let ours = input.nfkd();
        let theirs = un_reference::nfkd(input);
        assert_eq!(
            &*ours, &*theirs,
            "NFKD({:?}): simd={:?} un={:?}",
            input, &*ours, theirs
        );
    }
}
