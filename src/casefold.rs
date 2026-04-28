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
///
/// In `CaseFoldMode::Standard`, the implementation runs an ASCII fast path:
/// 64-byte chunks are scanned via the dispatched SIMD scanner with bound
/// `0x80`. Chunks with zero non-ASCII bytes get a scalar `mask | 0x20`-style
/// lowercase pass (no per-byte trie lookup), which dominates throughput on
/// ASCII / Latin-1 inputs. Non-ASCII chunks fall back to the per-codepoint
/// trie-driven path. Other modes (Turkish, etc.) skip the fast path because
/// their override rules apply within the ASCII range.
pub fn casefold<'a>(input: &'a str, mode: CaseFoldMode) -> Cow<'a, str> {
    if input.is_empty() {
        return Cow::Borrowed(input);
    }

    if mode == CaseFoldMode::Standard {
        casefold_ascii_fastpath(input)
    } else {
        casefold_scalar(input, mode)
    }
}

/// Scalar fallback used by both the non-Standard modes and the ASCII fast
/// path's tail / non-ASCII region. Walks codepoints through the casefold
/// trie; returns `Cow::Borrowed` if nothing changed.
fn casefold_scalar<'a>(input: &'a str, mode: CaseFoldMode) -> Cow<'a, str> {
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
            },
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

/// Standard-mode casefold with a 64-byte SIMD-driven ASCII fast path.
///
/// We walk the input in 64-byte chunks, scanning with `bound = 0x80` to
/// detect any non-ASCII byte. ASCII-only chunks are lowercased via the
/// scalar `0x41..=0x5A → +0x20` rule (no trie lookup, single byte per
/// position). The first chunk containing a non-ASCII byte switches the
/// remainder of the input over to the per-codepoint trie-driven path.
fn casefold_ascii_fastpath<'a>(input: &'a str) -> Cow<'a, str> {
    let bytes = input.as_bytes();
    let len = bytes.len();
    let ptr = bytes.as_ptr();

    // First scan: locate the first non-ASCII byte (if any) and the first
    // ASCII uppercase byte (if any), to decide whether allocation is needed.
    let mut pos = 0usize;
    let mut first_change: Option<usize> = None;

    // SIMD-driven ASCII probe: 64-byte chunks scanning for any byte >= 0x80.
    while pos + 64 <= len {
        // SAFETY: `pos + 64 <= len`, so the pointer is valid for 64 bytes.
        let nonascii = unsafe { crate::simd::scan_chunk(ptr.add(pos), 0x80) };
        if nonascii != 0 {
            // Non-ASCII somewhere in this chunk — break out and delegate to
            // the scalar path for the entire input. Trying to splice the
            // ASCII prefix here would not save work (the scalar path's own
            // pre-scan does the same prefix detection in tight scalar code).
            return casefold_scalar(input, CaseFoldMode::Standard);
        }
        // Pure-ASCII chunk: probe for an uppercase byte to decide whether
        // we even need to allocate. We use a second SIMD scan with bound
        // `0x41` (`'A'`) and refine in scalar if any byte >= 'A' exists,
        // since 'A'..='Z' is a tiny window inside [0x41, 0x80).
        let upper_or_more = unsafe { crate::simd::scan_chunk(ptr.add(pos), b'A') };
        if upper_or_more != 0 {
            // Some byte is >= 'A'. Find the first byte that is uppercase ASCII.
            let mut mask = upper_or_more;
            while mask != 0 {
                let bit = mask.trailing_zeros() as usize;
                mask &= mask.wrapping_sub(1);
                let b = bytes[pos + bit];
                if b.is_ascii_uppercase() {
                    first_change = Some(pos + bit);
                    break;
                }
            }
            if first_change.is_some() {
                break;
            }
        }
        pos += 64;
    }

    // Tail (or whole input if it's < 64 bytes): scan byte-by-byte for the
    // first uppercase ASCII or any non-ASCII byte.
    if first_change.is_none() {
        let mut tail = pos;
        while tail < len {
            let b = bytes[tail];
            if b >= 0x80 {
                // Hit a non-ASCII byte before finding any uppercase: defer to
                // scalar (it will re-scan, but the input is by definition
                // not pure ASCII so the SIMD fast path is exhausted anyway).
                return casefold_scalar(input, CaseFoldMode::Standard);
            }
            if b.is_ascii_uppercase() {
                first_change = Some(tail);
                break;
            }
            tail += 1;
        }
    }

    let Some(start) = first_change else {
        // Pure ASCII, no uppercase: borrowed.
        return Cow::Borrowed(input);
    };

    // We have a definite change at `start`. Build the output:
    //   - copy bytes [0, start) verbatim
    //   - lowercase bytes [start, ?) in scalar: `b | 0x20` for 'A'..='Z',
    //     copy others, until we hit either end-of-input or a non-ASCII byte.
    //   - if we hit a non-ASCII byte, append the per-codepoint folded tail.
    let mut out = String::with_capacity(len);
    // SAFETY: bytes [0, start) are pure ASCII (we only walked past them
    // when no byte was >= 0x80), so they are valid UTF-8.
    out.push_str(unsafe { core::str::from_utf8_unchecked(&bytes[..start]) });

    let mut i = start;
    while i < len {
        let b = bytes[i];
        if b >= 0x80 {
            // Switch to per-codepoint fallback for the rest of the input.
            // SAFETY: `i` is on a UTF-8 boundary because we only advanced
            // through ASCII bytes (each 1 byte wide) up to this point.
            let rest = unsafe { core::str::from_utf8_unchecked(&bytes[i..]) };
            for ch in rest.chars() {
                out.push(casefold_char(ch, CaseFoldMode::Standard));
            }
            return Cow::Owned(out);
        }
        if b.is_ascii_uppercase() {
            // Lowercase via OR with 0x20.
            out.push((b | 0x20) as char);
        } else {
            out.push(b as char);
        }
        i += 1;
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
        assert_eq!(
            casefold_char('\u{00C0}', CaseFoldMode::Standard),
            '\u{00E0}'
        );
        // U+00D6 Ö → U+00F6 ö
        assert_eq!(
            casefold_char('\u{00D6}', CaseFoldMode::Standard),
            '\u{00F6}'
        );
    }

    #[test]
    fn fold_greek() {
        // U+0391 Α → U+03B1 α
        assert_eq!(
            casefold_char('\u{0391}', CaseFoldMode::Standard),
            '\u{03B1}'
        );
        // U+03A3 Σ → U+03C3 σ
        assert_eq!(
            casefold_char('\u{03A3}', CaseFoldMode::Standard),
            '\u{03C3}'
        );
    }

    #[test]
    fn fold_cyrillic() {
        // U+0410 А → U+0430 а
        assert_eq!(
            casefold_char('\u{0410}', CaseFoldMode::Standard),
            '\u{0430}'
        );
    }

    #[test]
    fn fold_micro_sign() {
        // U+00B5 µ (MICRO SIGN) → U+03BC μ (GREEK SMALL LETTER MU)
        assert_eq!(
            casefold_char('\u{00B5}', CaseFoldMode::Standard),
            '\u{03BC}'
        );
    }

    #[test]
    fn fold_sharp_s() {
        // U+1E9E ẞ (LATIN CAPITAL LETTER SHARP S) → U+00DF ß
        assert_eq!(
            casefold_char('\u{1E9E}', CaseFoldMode::Standard),
            '\u{00DF}'
        );
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
        let result = casefold(
            "abcdefghijklmnopqrstuvwxyz0123456789",
            CaseFoldMode::Standard,
        );
        assert!(matches!(result, Cow::Borrowed(_)));
    }
}
