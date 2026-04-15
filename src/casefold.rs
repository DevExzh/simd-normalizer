//! Unicode simple case folding (CaseFolding.txt, status C+S).
//!
//! Provides character-level and string-level case folding for case-insensitive
//! matching. Supports both standard folding and Turkish/Azerbaijani locale mode.

use alloc::borrow::Cow;
use alloc::string::String;

use crate::tables;

/// Case folding mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaseFoldMode {
    /// Standard Unicode case folding (CaseFolding.txt status C+S).
    Standard,
    /// Turkish/Azerbaijani locale folding.
    ///
    /// Overrides:
    /// - U+0049 (I) → U+0131 (ı) instead of U+0069 (i)
    /// - U+0130 (İ) → U+0069 (i) instead of standard mapping
    Turkish,
}

/// Fold a single character using simple case folding.
///
/// Returns the folded character, or the input character unchanged if no
/// folding applies.
#[inline]
pub fn casefold_char(c: char, mode: CaseFoldMode) -> char {
    // Turkish exceptions override the standard mapping.
    if mode == CaseFoldMode::Turkish
        && let Some(folded) = tables::turkish_casefold(c)
    {
        return folded;
    }
    tables::lookup_casefold(c).unwrap_or(c)
}

/// Fold a string using simple case folding.
///
/// Returns `Cow::Borrowed` if the string is already fully case-folded
/// (no characters changed).
pub fn casefold<'a>(input: &'a str, mode: CaseFoldMode) -> Cow<'a, str> {
    if input.is_empty() {
        return Cow::Borrowed(input);
    }

    // Quick scan: find first character that would change.
    let mut scan_iter = input.char_indices();
    let first_change = loop {
        match scan_iter.next() {
            None => return Cow::Borrowed(input),
            Some((idx, ch)) => {
                let folded = casefold_char(ch, mode);
                if folded != ch {
                    break idx;
                }
            }
        }
    };

    // Build the output: copy unchanged prefix, then fold the rest.
    let mut out = String::with_capacity(input.len());
    out.push_str(&input[..first_change]);

    for ch in input[first_change..].chars() {
        out.push(casefold_char(ch, mode));
    }

    Cow::Owned(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Character-level tests ----

    #[test]
    fn fold_ascii_uppercase() {
        assert_eq!(casefold_char('A', CaseFoldMode::Standard), 'a');
        assert_eq!(casefold_char('Z', CaseFoldMode::Standard), 'z');
    }

    #[test]
    fn fold_ascii_lowercase_unchanged() {
        assert_eq!(casefold_char('a', CaseFoldMode::Standard), 'a');
        assert_eq!(casefold_char('z', CaseFoldMode::Standard), 'z');
    }

    #[test]
    fn fold_digit_unchanged() {
        assert_eq!(casefold_char('0', CaseFoldMode::Standard), '0');
        assert_eq!(casefold_char('9', CaseFoldMode::Standard), '9');
    }

    #[test]
    fn fold_latin_extended() {
        // U+00C0 À → U+00E0 à
        assert_eq!(casefold_char('\u{00C0}', CaseFoldMode::Standard), '\u{00E0}');
        // U+00D6 Ö → U+00F6 ö
        assert_eq!(casefold_char('\u{00D6}', CaseFoldMode::Standard), '\u{00F6}');
    }

    #[test]
    fn fold_greek() {
        // U+0391 Α → U+03B1 α
        assert_eq!(casefold_char('\u{0391}', CaseFoldMode::Standard), '\u{03B1}');
        // U+03A3 Σ → U+03C3 σ
        assert_eq!(casefold_char('\u{03A3}', CaseFoldMode::Standard), '\u{03C3}');
    }

    #[test]
    fn fold_cyrillic() {
        // U+0410 А → U+0430 а
        assert_eq!(casefold_char('\u{0410}', CaseFoldMode::Standard), '\u{0430}');
    }

    #[test]
    fn fold_micro_sign() {
        // U+00B5 µ (MICRO SIGN) → U+03BC μ (GREEK SMALL LETTER MU)
        assert_eq!(casefold_char('\u{00B5}', CaseFoldMode::Standard), '\u{03BC}');
    }

    #[test]
    fn fold_sharp_s() {
        // U+1E9E ẞ (LATIN CAPITAL LETTER SHARP S) → U+00DF ß
        assert_eq!(casefold_char('\u{1E9E}', CaseFoldMode::Standard), '\u{00DF}');
    }

    // ---- Turkish mode ----

    #[test]
    fn fold_turkish_dotless_i() {
        // Standard: I → i
        assert_eq!(casefold_char('I', CaseFoldMode::Standard), 'i');
        // Turkish: I → ı (U+0131)
        assert_eq!(casefold_char('I', CaseFoldMode::Turkish), '\u{0131}');
    }

    #[test]
    fn fold_turkish_dotted_capital_i() {
        // Turkish: İ (U+0130) → i
        assert_eq!(casefold_char('\u{0130}', CaseFoldMode::Turkish), 'i');
    }

    #[test]
    fn fold_turkish_other_chars_unchanged() {
        // Non-I characters should fold the same in Turkish mode.
        assert_eq!(casefold_char('A', CaseFoldMode::Turkish), 'a');
        assert_eq!(casefold_char('a', CaseFoldMode::Turkish), 'a');
    }

    // ---- String-level tests ----

    #[test]
    fn fold_string_ascii() {
        let result = casefold("Hello World", CaseFoldMode::Standard);
        assert_eq!(&*result, "hello world");
    }

    #[test]
    fn fold_string_already_folded() {
        let result = casefold("hello world", CaseFoldMode::Standard);
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(&*result, "hello world");
    }

    #[test]
    fn fold_string_empty() {
        let result = casefold("", CaseFoldMode::Standard);
        assert!(matches!(result, Cow::Borrowed(_)));
    }

    #[test]
    fn fold_string_mixed() {
        let result = casefold("Ströme", CaseFoldMode::Standard);
        assert_eq!(&*result, "ströme");
    }

    #[test]
    fn fold_string_turkish() {
        let result = casefold("Istanbul", CaseFoldMode::Turkish);
        // I → ı in Turkish mode
        assert_eq!(&*result, "\u{0131}stanbul");
    }

    #[test]
    fn fold_string_all_ascii_lowercase() {
        // Should return borrowed.
        let result = casefold("abcdefghijklmnopqrstuvwxyz0123456789", CaseFoldMode::Standard);
        assert!(matches!(result, Cow::Borrowed(_)));
    }
}
