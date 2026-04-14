//! Canonical and compatible decomposition engine.
//!
//! Characters are recursively decomposed, with results collected into a
//! CccBuffer for subsequent canonical ordering. Hangul syllables are
//! decomposed algorithmically; all other characters go through the trie.

use crate::ccc::CccBuffer;
use crate::hangul;
use crate::tables::{self, DecompResult};

/// Which decomposition form to use.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum DecompForm {
    Canonical,
    Compatible,
}

/// Look up the decomposition result and CCC for a character.
#[inline]
fn lookup_decomp(c: char, form: DecompForm) -> (DecompResult, u8) {
    match form {
        DecompForm::Canonical => tables::lookup_canonical_decomp(c),
        DecompForm::Compatible => tables::lookup_compat_decomp(c),
    }
}

/// Get expansion data slice for a decomposition result.
#[inline]
fn expansion_data(offset: usize, length: usize, form: DecompForm) -> &'static [u16] {
    match form {
        DecompForm::Canonical => tables::canonical_expansion_data(offset, length),
        DecompForm::Compatible => tables::compat_expansion_data(offset, length),
    }
}

/// Recursively decompose a character, appending results to the CccBuffer.
#[inline]
pub(crate) fn decompose(c: char, output: &mut CccBuffer, form: DecompForm) {
    // Fast path: 7-bit ASCII never decomposes.
    if (c as u32) <= 0x7F {
        output.push(c, 0);
        return;
    }

    // Hangul syllables: algorithmic decomposition.
    if hangul::is_hangul_syllable(c) {
        let (l, v, t) = hangul::decompose_hangul(c);
        output.push(l, 0);
        output.push(v, 0);
        if let Some(t_char) = t {
            output.push(t_char, 0);
        }
        return;
    }

    // Trie lookup.
    let (decomp, ccc) = lookup_decomp(c, form);
    match decomp {
        DecompResult::None => {
            // Character maps to itself. CCC is correctly encoded in the
            // decomposition trie for all characters (the table generator
            // stores it for both decomposing and non-decomposing code points).
            output.push(c, ccc);
        }
        DecompResult::Singleton(decomposed) => {
            // Recursively decompose the singleton.
            decompose(decomposed, output, form);
        }
        DecompResult::Expansion { offset, length } => {
            // Fetch expansion data and iterate, decoding surrogate pairs.
            let data = expansion_data(offset, length, form);
            let mut i = 0;
            while i < data.len() {
                let unit = data[i];
                let ch = if (0xD800..=0xDBFF).contains(&unit) && i + 1 < data.len() {
                    // High surrogate: decode pair
                    let high = unit;
                    let low = data[i + 1];
                    if (0xDC00..=0xDFFF).contains(&low) {
                        let cp = ((high as u32 - 0xD800) << 10) + (low as u32 - 0xDC00) + 0x10000;
                        i += 2;
                        // SAFETY: cp is a valid Unicode scalar value constructed
                        // from a valid surrogate pair in our generated tables.
                        debug_assert!(cp <= 0x10FFFF && !(0xD800..=0xDFFF).contains(&cp));
                        unsafe { char::from_u32_unchecked(cp) }
                    } else {
                        // Malformed: treat high surrogate as-is (shouldn't happen)
                        i += 1;
                        continue;
                    }
                } else {
                    i += 1;
                    // SAFETY: BMP code points from the table are valid.
                    debug_assert!(unit <= 0xD7FF || (0xE000..=0xFFFF).contains(&unit));
                    unsafe { char::from_u32_unchecked(unit as u32) }
                };
                // Recursively decompose each character from the expansion.
                decompose(ch, output, form);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ccc::CharAndCcc;
    use alloc::vec::Vec;

    fn decompose_to_vec(c: char, form: DecompForm) -> Vec<CharAndCcc> {
        let mut buf = CccBuffer::new();
        decompose(c, &mut buf, form);
        buf.as_slice().to_vec()
    }

    // --- ASCII passthrough ---

    #[test]
    fn test_ascii_passthrough() {
        for cp in 0x20u32..=0x7E {
            let c = char::from_u32(cp).unwrap();
            let result = decompose_to_vec(c, DecompForm::Canonical);
            assert_eq!(result.len(), 1, "ASCII U+{cp:04X} should not decompose");
            assert_eq!(result[0].ch, c);
            assert_eq!(result[0].ccc, 0);
        }
    }

    #[test]
    fn test_nul_passthrough() {
        let result = decompose_to_vec('\0', DecompForm::Canonical);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].ch, '\0');
    }

    // --- Hangul syllable decomposition ---

    #[test]
    fn test_hangul_lv() {
        let result = decompose_to_vec('\u{AC00}', DecompForm::Canonical);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].ch, '\u{1100}');
        assert_eq!(result[1].ch, '\u{1161}');
    }

    #[test]
    fn test_hangul_lvt() {
        let result = decompose_to_vec('\u{AC01}', DecompForm::Canonical);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].ch, '\u{1100}');
        assert_eq!(result[1].ch, '\u{1161}');
        assert_eq!(result[2].ch, '\u{11A8}');
    }

    // --- Trie-based decomposition (real Unicode data) ---

    #[test]
    fn test_a_grave_nfd() {
        // U+00C0 A with grave -> U+0041 A + U+0300 combining grave
        let result = decompose_to_vec('\u{00C0}', DecompForm::Canonical);
        assert!(
            result.len() >= 2,
            "A-grave should decompose: got {:?}",
            result
        );
        assert_eq!(result[0].ch, 'A');
        assert_eq!(result[0].ccc, 0);
        assert_eq!(result[1].ch, '\u{0300}');
        assert_eq!(result[1].ccc, 230);
    }

    #[test]
    fn test_e_acute_nfd() {
        // U+00E9 e with acute -> U+0065 e + U+0301 combining acute
        let result = decompose_to_vec('\u{00E9}', DecompForm::Canonical);
        assert!(
            result.len() >= 2,
            "e-acute should decompose: got {:?}",
            result
        );
        assert_eq!(result[0].ch, 'e');
        assert_eq!(result[1].ch, '\u{0301}');
    }

    #[test]
    fn test_combining_mark_passthrough() {
        // U+0300 combining grave: no decomposition, CCC=230
        let result = decompose_to_vec('\u{0300}', DecompForm::Canonical);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].ch, '\u{0300}');
        assert_eq!(result[0].ccc, 230);
    }

    #[test]
    fn test_cjk_passthrough() {
        let result = decompose_to_vec('\u{4E00}', DecompForm::Canonical);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].ch, '\u{4E00}');
        assert_eq!(result[0].ccc, 0);
    }

    #[test]
    fn test_emoji_passthrough() {
        let result = decompose_to_vec('\u{1F600}', DecompForm::Canonical);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].ch, '\u{1F600}');
    }

    // --- Buffer accumulation ---

    #[test]
    fn test_multiple_chars_into_buffer() {
        let mut buf = CccBuffer::new();
        decompose('A', &mut buf, DecompForm::Canonical);
        decompose('\u{AC00}', &mut buf, DecompForm::Canonical); // -> 2 jamos
        decompose('B', &mut buf, DecompForm::Canonical);
        let slice = buf.as_slice();
        assert_eq!(slice.len(), 4);
        assert_eq!(slice[0].ch, 'A');
        assert_eq!(slice[1].ch, '\u{1100}');
        assert_eq!(slice[2].ch, '\u{1161}');
        assert_eq!(slice[3].ch, 'B');
    }
}
