//! Integration tests for Unicode case folding.

use simd_normalizer::{CaseFoldMode, casefold, casefold_char};
use std::borrow::Cow;

// ---------------------------------------------------------------------------
// Character-level integration tests
// ---------------------------------------------------------------------------

#[test]
fn ascii_uppercase_folds_to_lowercase() {
    for c in 'A'..='Z' {
        let folded = casefold_char(c, CaseFoldMode::Standard);
        let expected = (c as u8 + 32) as char;
        assert_eq!(folded, expected, "Expected {:?} -> {:?}", c, expected);
    }
}

#[test]
fn ascii_lowercase_unchanged() {
    for c in 'a'..='z' {
        assert_eq!(casefold_char(c, CaseFoldMode::Standard), c);
    }
}

#[test]
fn digits_and_symbols_unchanged() {
    for c in '0'..='9' {
        assert_eq!(casefold_char(c, CaseFoldMode::Standard), c);
    }
    for &c in &['!', '@', '#', '$', '%', '^', '&', '*', '(', ')'] {
        assert_eq!(casefold_char(c, CaseFoldMode::Standard), c);
    }
}

#[test]
fn latin_extended_folding() {
    // A sample of Latin Extended characters with known foldings.
    let cases = [
        ('\u{00C0}', '\u{00E0}'), // À → à
        ('\u{00D6}', '\u{00F6}'), // Ö → ö
        ('\u{00DC}', '\u{00FC}'), // Ü → ü
        ('\u{00C9}', '\u{00E9}'), // É → é
        ('\u{00D1}', '\u{00F1}'), // Ñ → ñ
    ];
    for (upper, lower) in cases {
        assert_eq!(
            casefold_char(upper, CaseFoldMode::Standard),
            lower,
            "U+{:04X} should fold to U+{:04X}",
            upper as u32,
            lower as u32,
        );
    }
}

#[test]
fn greek_folding() {
    let cases = [
        ('\u{0391}', '\u{03B1}'), // Α → α
        ('\u{0392}', '\u{03B2}'), // Β → β
        ('\u{03A3}', '\u{03C3}'), // Σ → σ
        ('\u{03A9}', '\u{03C9}'), // Ω → ω
    ];
    for (upper, lower) in cases {
        assert_eq!(
            casefold_char(upper, CaseFoldMode::Standard),
            lower,
            "Greek U+{:04X} should fold to U+{:04X}",
            upper as u32,
            lower as u32,
        );
    }
}

#[test]
fn cyrillic_folding() {
    let cases = [
        ('\u{0410}', '\u{0430}'), // А → а
        ('\u{0411}', '\u{0431}'), // Б → б
        ('\u{042F}', '\u{044F}'), // Я → я
    ];
    for (upper, lower) in cases {
        assert_eq!(
            casefold_char(upper, CaseFoldMode::Standard),
            lower,
            "Cyrillic U+{:04X} should fold to U+{:04X}",
            upper as u32,
            lower as u32,
        );
    }
}

#[test]
fn special_case_foldings() {
    // MICRO SIGN → GREEK SMALL LETTER MU
    assert_eq!(
        casefold_char('\u{00B5}', CaseFoldMode::Standard),
        '\u{03BC}'
    );
    // LATIN CAPITAL LETTER SHARP S → LATIN SMALL LETTER SHARP S
    assert_eq!(
        casefold_char('\u{1E9E}', CaseFoldMode::Standard),
        '\u{00DF}'
    );
}

// ---------------------------------------------------------------------------
// Turkish mode
// ---------------------------------------------------------------------------

#[test]
fn turkish_capital_i_to_dotless() {
    // Standard: I → i
    assert_eq!(casefold_char('I', CaseFoldMode::Standard), 'i');
    // Turkish: I → ı (U+0131)
    assert_eq!(casefold_char('I', CaseFoldMode::Turkish), '\u{0131}');
}

#[test]
fn turkish_dotted_capital_i_to_i() {
    // Turkish: İ (U+0130) → i
    assert_eq!(casefold_char('\u{0130}', CaseFoldMode::Turkish), 'i');
}

#[test]
fn turkish_other_chars_same_as_standard() {
    // Non-I characters should behave identically in Turkish mode.
    for c in 'A'..='H' {
        assert_eq!(
            casefold_char(c, CaseFoldMode::Turkish),
            casefold_char(c, CaseFoldMode::Standard),
        );
    }
    for c in 'J'..='Z' {
        assert_eq!(
            casefold_char(c, CaseFoldMode::Turkish),
            casefold_char(c, CaseFoldMode::Standard),
        );
    }
}

// ---------------------------------------------------------------------------
// String-level integration tests
// ---------------------------------------------------------------------------

#[test]
fn string_already_folded_returns_borrowed() {
    let result = casefold("hello world", CaseFoldMode::Standard);
    assert!(matches!(result, Cow::Borrowed(_)));
}

#[test]
fn string_empty_returns_borrowed() {
    let result = casefold("", CaseFoldMode::Standard);
    assert!(matches!(result, Cow::Borrowed(_)));
}

#[test]
fn string_mixed_case() {
    assert_eq!(
        &*casefold("Hello World", CaseFoldMode::Standard),
        "hello world"
    );
    assert_eq!(&*casefold("HELLO", CaseFoldMode::Standard), "hello");
}

#[test]
fn string_unicode_mixed() {
    assert_eq!(&*casefold("Ströme", CaseFoldMode::Standard), "ströme");
    assert_eq!(&*casefold("CAFÉ", CaseFoldMode::Standard), "café");
}

#[test]
fn string_turkish_mode() {
    let result = casefold("Istanbul", CaseFoldMode::Turkish);
    // I → ı in Turkish mode
    assert_eq!(&*result, "\u{0131}stanbul");
}

// ---------------------------------------------------------------------------
// BMP scan: verify no panics across the entire BMP
// ---------------------------------------------------------------------------

#[test]
fn bmp_scan_no_panics() {
    for cp in 0u32..=0xFFFF {
        if let Some(c) = char::from_u32(cp) {
            let _ = casefold_char(c, CaseFoldMode::Standard);
            let _ = casefold_char(c, CaseFoldMode::Turkish);
        }
    }
}

#[test]
fn supplementary_sample_no_panics() {
    // Sample supplementary code points
    let cps = [0x10000u32, 0x10400, 0x10428, 0x1D400, 0x1F600, 0x10FFFF];
    for &cp in &cps {
        if let Some(c) = char::from_u32(cp) {
            let _ = casefold_char(c, CaseFoldMode::Standard);
        }
    }
}

#[test]
fn casefold_idempotent() {
    // Case folding applied twice should produce the same result as once.
    let inputs = [
        "Hello World",
        "CAFÉ",
        "Ströme",
        "Istanbul",
        "\u{0391}\u{0392}\u{0393}", // Greek uppercase
        "\u{0410}\u{0411}\u{0412}", // Cyrillic uppercase
    ];
    for input in &inputs {
        let once = casefold(input, CaseFoldMode::Standard);
        let twice = casefold(&once, CaseFoldMode::Standard);
        assert_eq!(&*once, &*twice, "casefold not idempotent for {:?}", input);
    }
}
