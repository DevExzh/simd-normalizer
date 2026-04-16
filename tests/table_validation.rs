//! Exhaustive validation tests for case folding tables and confusable mappings.
//!
//! These tests verify structural invariants across the full Unicode range,
//! port select ICU4X case-mapping tests, and validate confusable skeleton
//! properties exhaustively.

use simd_normalizer::UnicodeNormalization;
use simd_normalizer::{CaseFoldMode, are_confusable, casefold, casefold_char, skeleton};

// ===========================================================================
// Case Folding — ICU4X ported tests
// ===========================================================================

#[test]
fn icu4x_deseret_supplementary_fold() {
    // ICU4X conversions.rs: Deseret capital LONG I (U+10414) → small (U+1043C)
    let upper = '\u{10414}';
    let lower = '\u{1043C}';
    assert_eq!(
        casefold_char(upper, CaseFoldMode::Standard),
        lower,
        "Deseret U+10414 should fold to U+1043C"
    );
    // Already-lowercase Deseret stays unchanged.
    assert_eq!(
        casefold_char(lower, CaseFoldMode::Standard),
        lower,
        "Deseret U+1043C should remain U+1043C"
    );
}

#[test]
fn icu4x_titlecase_character_fold() {
    // ICU4X conversions.rs: Titlecase digraph DZ (U+01C4 Ǆ) should fold to
    // lowercase; the lowercase form (U+01C6 ǆ) should be unchanged.
    let titlecase = '\u{01C4}'; // Ǆ (DZ)
    let lowercase = '\u{01C6}'; // ǆ (dz)
    let folded = casefold_char(titlecase, CaseFoldMode::Standard);
    assert_eq!(
        folded, lowercase,
        "U+01C4 (Ǆ) should fold to U+01C6 (ǆ), got U+{:04X}",
        folded as u32
    );
    assert_eq!(
        casefold_char(lowercase, CaseFoldMode::Standard),
        lowercase,
        "U+01C6 (ǆ) should remain unchanged"
    );
}

#[test]
fn icu4x_greek_case_insensitive_match() {
    // ICU4X conversions.rs: fold(uppercase_greek) == fold(lowercase_greek)
    let upper = "ΙΕΣΥΣ ΧΡΙΣΤΟΣ";
    let lower = "ιεσυς χριστος";
    let folded_upper = casefold(upper, CaseFoldMode::Standard);
    let folded_lower = casefold(lower, CaseFoldMode::Standard);
    assert_eq!(
        &*folded_upper, &*folded_lower,
        "Greek uppercase and lowercase should fold to the same string"
    );
}

// ===========================================================================
// Case Folding — BMP exhaustive idempotence
// ===========================================================================

#[test]
fn bmp_casefold_idempotent() {
    // For every BMP codepoint, fold(fold(c)) == fold(c).
    let mut failures = Vec::new();
    for cp in 0u32..=0xFFFF {
        if let Some(c) = char::from_u32(cp) {
            let once = casefold_char(c, CaseFoldMode::Standard);
            let twice = casefold_char(once, CaseFoldMode::Standard);
            if once != twice {
                failures.push(format!(
                    "U+{:04X}: fold=U+{:04X}, fold(fold)=U+{:04X}",
                    cp, once as u32, twice as u32
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "BMP casefold idempotence failures ({}):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

// ===========================================================================
// Case Folding — Supplementary exhaustive idempotence
// ===========================================================================

#[test]
fn supplementary_casefold_idempotent() {
    // For every supplementary codepoint, fold(fold(c)) == fold(c).
    let mut failures = Vec::new();
    for cp in 0x10000u32..=0x10FFFF {
        if let Some(c) = char::from_u32(cp) {
            let once = casefold_char(c, CaseFoldMode::Standard);
            let twice = casefold_char(once, CaseFoldMode::Standard);
            if once != twice {
                failures.push(format!(
                    "U+{:04X}: fold=U+{:04X}, fold(fold)=U+{:04X}",
                    cp, once as u32, twice as u32
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "Supplementary casefold idempotence failures ({}):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

// ===========================================================================
// Case Folding — Turkish mode only differs on I/İ
// ===========================================================================

#[test]
fn turkish_mode_only_differs_on_i_dotted_i() {
    // For every BMP char except U+0049 (I) and U+0130 (İ),
    // Turkish fold == Standard fold.
    let mut failures = Vec::new();
    for cp in 0u32..=0xFFFF {
        if cp == 0x0049 || cp == 0x0130 {
            continue;
        }
        if let Some(c) = char::from_u32(cp) {
            let standard = casefold_char(c, CaseFoldMode::Standard);
            let turkish = casefold_char(c, CaseFoldMode::Turkish);
            if standard != turkish {
                failures.push(format!(
                    "U+{:04X}: standard=U+{:04X}, turkish=U+{:04X}",
                    cp, standard as u32, turkish as u32
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "Turkish mode differs from standard for chars other than I/İ ({}):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

// ===========================================================================
// Case Folding — String fold matches char-by-char fold
// ===========================================================================

#[test]
fn string_fold_matches_char_by_char() {
    let test_strings = [
        "Hello World",
        "CAFE\u{0301}",   // CAFÉ with combining accent
        "Stro\u{0308}me", // Ströme with combining diaeresis
        "Istanbul",
        "\u{0391}\u{0392}\u{0393}\u{0394}", // Greek uppercase
        "\u{0410}\u{0411}\u{0412}\u{0413}", // Cyrillic uppercase
        "\u{10414}\u{10415}\u{10416}",      // Deseret uppercase
        "\u{01C4}\u{01C7}\u{01CA}",         // Titlecase digraphs DZ, LJ, NJ
        "\u{00B5}\u{1E9E}\u{017F}",         // Micro sign, capital sharp S, long S
        "The quick BROWN Fox",
        "\u{0130}stanbul",             // İstanbul
        "\u{1D400}\u{1D401}\u{1D402}", // Math bold A, B, C
        "",
        "already lowercase",
    ];

    for mode in [CaseFoldMode::Standard, CaseFoldMode::Turkish] {
        for &s in &test_strings {
            let api_result = casefold(s, mode);
            let manual: String = s.chars().map(|c| casefold_char(c, mode)).collect();
            assert_eq!(
                &*api_result, &manual,
                "String fold != char-by-char fold for {:?} (mode={:?})",
                s, mode
            );
        }
    }
}

// ===========================================================================
// Case Folding — NFC interaction
// ===========================================================================

#[test]
fn casefold_nfc_interaction() {
    // casefold(NFC(s)) should be canonically equivalent to casefold(s).
    // Simple case folding is a character-by-character operation, so the results
    // may differ in normalization form. We compare after NFC-normalizing both.
    let test_strings = [
        "Hello World",
        "CAFE\u{0301}", // decomposed E-acute sequence
        "\u{00C9}cole", // precomposed E-acute
        "\u{0391}\u{0392}\u{0393}",
        "\u{0410}\u{0411}\u{0412}",
        "abcdef",
        "\u{00B5}",
        "\u{1E9E}",
        "\u{01C4}",
        "Istanbul",
    ];

    let mut failures = Vec::new();
    for &s in &test_strings {
        let nfc_s = s.nfc();
        let fold_original = casefold(s, CaseFoldMode::Standard);
        let fold_nfc = casefold(&nfc_s, CaseFoldMode::Standard);
        // Compare after NFC normalization of both results.
        let nfc_fold_original = fold_original.nfc();
        let nfc_fold_nfc = fold_nfc.nfc();
        if *nfc_fold_original != *nfc_fold_nfc {
            failures.push(format!(
                "Mismatch for {:?}: NFC(fold(s))={:?}, NFC(fold(NFC(s)))={:?}",
                s, &*nfc_fold_original, &*nfc_fold_nfc
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "casefold + NFC interaction failures ({}):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

// ===========================================================================
// Confusable — Full BMP skeleton no-panic (every codepoint, step 1)
// ===========================================================================

#[test]
fn bmp_skeleton_no_panic_exhaustive() {
    // Every single BMP codepoint (not step 16 like existing test).
    let mut buf = String::new();
    for cp in 0u32..=0xFFFF {
        if let Some(c) = char::from_u32(cp) {
            buf.clear();
            buf.push(c);
            let _ = skeleton(&buf);
        }
    }
}

// ===========================================================================
// Confusable — Supplementary skeleton no-panic
// ===========================================================================

#[test]
fn supplementary_skeleton_no_panic() {
    // Every supplementary codepoint, step 100 to keep test fast.
    let mut buf = String::new();
    for cp in (0x10000u32..=0x10FFFF).step_by(100) {
        if let Some(c) = char::from_u32(cp) {
            buf.clear();
            buf.push(c);
            let _ = skeleton(&buf);
        }
    }
}

// ===========================================================================
// Confusable — Skeleton convergence exhaustive
// ===========================================================================

#[test]
fn skeleton_convergence_exhaustive() {
    // For every 10th BMP codepoint, skeleton(skeleton(c)) == skeleton(skeleton(skeleton(c))).
    let mut failures = Vec::new();
    let mut buf = String::new();
    for cp in (0u32..=0xFFFF).step_by(10) {
        if let Some(c) = char::from_u32(cp) {
            buf.clear();
            buf.push(c);
            let s1 = skeleton(&buf);
            let s2 = skeleton(&s1);
            let s3 = skeleton(&s2);
            if s2 != s3 {
                failures.push(format!("U+{:04X}: skeleton^2 != skeleton^3", cp));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "Skeleton convergence failures ({}):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

// ===========================================================================
// Confusable — are_confusable reflexivity
// ===========================================================================

#[test]
fn are_confusable_reflexivity() {
    // For every 100th BMP char, are_confusable(s, s) == true.
    let mut failures = Vec::new();
    let mut buf = String::new();
    for cp in (0u32..=0xFFFF).step_by(100) {
        if let Some(c) = char::from_u32(cp) {
            buf.clear();
            buf.push(c);
            if !are_confusable(&buf, &buf) {
                failures.push(format!("U+{:04X}: not reflexive", cp));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "are_confusable reflexivity failures ({}):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

// ===========================================================================
// Confusable — are_confusable symmetry
// ===========================================================================

#[test]
fn are_confusable_symmetry() {
    // For known confusable pairs, are_confusable(a, b) == are_confusable(b, a).
    let pairs: &[(&str, &str)] = &[
        ("a", "\u{0430}"),                              // Latin a / Cyrillic а
        ("e", "\u{0435}"),                              // Latin e / Cyrillic е
        ("o", "\u{043E}"),                              // Latin o / Cyrillic о
        ("p", "\u{0440}"),                              // Latin p / Cyrillic р
        ("o", "\u{03BF}"),                              // Latin o / Greek ο
        ("B", "\u{0392}"),                              // Latin B / Greek Β
        ("H", "\u{0397}"),                              // Latin H / Greek Η
        ("T", "\u{03A4}"),                              // Latin T / Greek Τ
        ("apple", "\u{0430}\u{0440}\u{0440}l\u{0435}"), // word-level
        ("paypal", "p\u{0430}yp\u{0430}l"),             // mixed-script word
    ];

    let mut failures = Vec::new();
    for &(a, b) in pairs {
        let ab = are_confusable(a, b);
        let ba = are_confusable(b, a);
        if ab != ba {
            failures.push(format!("({:?}, {:?}): a,b={} but b,a={}", a, b, ab, ba));
        }
    }
    assert!(
        failures.is_empty(),
        "are_confusable symmetry failures ({}):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

// ===========================================================================
// Confusable — Known confusable pairs (expanded set)
// ===========================================================================

#[test]
fn known_confusable_pairs_expanded() {
    // Expanded set of Latin/Greek/Cyrillic homoglyphs beyond existing tests.
    let confusable_pairs: &[(&str, &str, &str)] = &[
        // (left, right, description)
        ("o", "\u{03BF}", "Latin o / Greek omicron"),
        ("O", "\u{039F}", "Latin O / Greek Omicron"),
        ("B", "\u{0392}", "Latin B / Greek Beta"),
        ("H", "\u{0397}", "Latin H / Greek Eta"),
        ("T", "\u{03A4}", "Latin T / Greek Tau"),
        ("A", "\u{0391}", "Latin A / Greek Alpha"),
        ("E", "\u{0395}", "Latin E / Greek Epsilon"),
        ("K", "\u{039A}", "Latin K / Greek Kappa"),
        ("M", "\u{039C}", "Latin M / Greek Mu"),
        ("N", "\u{039D}", "Latin N / Greek Nu"),
        ("P", "\u{03A1}", "Latin P / Greek Rho"),
        ("X", "\u{03A7}", "Latin X / Greek Chi"),
        ("Y", "\u{03A5}", "Latin Y / Greek Upsilon"),
        ("Z", "\u{0396}", "Latin Z / Greek Zeta"),
        ("a", "\u{0430}", "Latin a / Cyrillic a"),
        ("e", "\u{0435}", "Latin e / Cyrillic ie"),
        ("o", "\u{043E}", "Latin o / Cyrillic o"),
        ("p", "\u{0440}", "Latin p / Cyrillic er"),
        ("c", "\u{0441}", "Latin c / Cyrillic es"),
        ("x", "\u{0445}", "Latin x / Cyrillic kha"),
        ("y", "\u{0443}", "Latin y / Cyrillic u"),
    ];

    let mut failures = Vec::new();
    for &(a, b, desc) in confusable_pairs {
        if !are_confusable(a, b) {
            failures.push(format!(
                "{}: {:?} and {:?} should be confusable",
                desc, a, b
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "Known confusable pair failures ({}):\n{}",
        failures.len(),
        failures.join("\n")
    );
}
