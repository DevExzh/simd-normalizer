//! Canonical and compatible decomposition engine.
//!
//! Characters are recursively decomposed, with results collected into a
//! CccBuffer for subsequent canonical ordering. Hangul syllables are
//! decomposed algorithmically; all other characters go through the trie.

use crate::ccc::CccBuffer;
use crate::hangul;
use crate::tables::{self, DecompResult};

/// Which decomposition form to use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DecompForm {
    Canonical,
    Compatible,
}

/// Look up the decomposition result and CCC for a character.
#[allow(dead_code)]
#[inline]
fn lookup_decomp(c: char, form: DecompForm) -> (DecompResult, u8) {
    match form {
        DecompForm::Canonical => tables::lookup_canonical_decomp(c),
        DecompForm::Compatible => tables::lookup_compat_decomp(c),
    }
}

/// Get expansion data slice for a decomposition result.
#[inline]
fn expansion_data(offset: usize, length: usize, form: DecompForm) -> &'static [u32] {
    match form {
        DecompForm::Canonical => tables::canonical_expansion_data(offset, length),
        DecompForm::Compatible => tables::compat_expansion_data(offset, length),
    }
}

/// Check if a code point is a CJK Unified Ideograph (CCC=0, no decomposition).
#[allow(dead_code)]
#[inline(always)]
fn is_cjk_ideograph(cp: u32) -> bool {
    // CJK Unified Ideographs + Extension A (BMP)
    (0x4E00..=0x9FFF).contains(&cp)
        || (0x3400..=0x4DBF).contains(&cp)
        // CJK Extensions B-F (supplementary, excludes U+2F800-2FA1F compat ideographs)
        || (0x20000..=0x2EBE0).contains(&cp)
        // CJK Extensions G-J (supplementary)
        || (0x30000..=0x323AF).contains(&cp)
}

/// Push a fully-decomposed character with its CCC.
///
/// The table generator pre-computes full recursive decompositions, so
/// characters appearing in singleton/expansion results are final — they
/// do not decompose further. We just need their CCC for canonical ordering.
#[inline(always)]
fn push_final_char(ch: char, output: &mut CccBuffer) {
    if (ch as u32) <= 0x7F {
        output.push(ch, 0);
    } else {
        output.push(ch, tables::lookup_ccc(ch));
    }
}

/// Decode expansion data (u32 entries with packed CCC + code point) and push
/// each character with its CCC. No recursive decomposition — characters in
/// expansion data are already fully decomposed by the table generator.
/// No separate CCC trie lookup needed — CCC is embedded in each entry.
#[inline]
fn decode_expansion(offset: usize, length: usize, form: DecompForm, output: &mut CccBuffer) {
    let data = expansion_data(offset, length, form);
    for &entry in data {
        let cp = entry & tables::EXPANSION_CP_MASK;
        let ccc = (entry >> tables::EXPANSION_CCC_SHIFT) as u8;
        // SAFETY: cp is a valid Unicode scalar value from our generated tables.
        debug_assert!(cp <= 0x10FFFF && !(0xD800..=0xDFFF).contains(&cp));
        let ch = unsafe { char::from_u32_unchecked(cp) };
        output.push(ch, ccc);
    }
}

/// Decompose a character, appending results to the CccBuffer.
///
/// Used by tests and as the standalone decomposition entry point.
/// The main normalization path uses `decompose_from_trie_value` instead.
#[allow(dead_code)]
#[inline]
pub(crate) fn decompose(c: char, output: &mut CccBuffer, form: DecompForm) {
    // Fast path: 7-bit ASCII never decomposes.
    if (c as u32) <= 0x7F {
        output.push(c, 0);
        return;
    }

    // Fast path: CJK Unified Ideographs never decompose (CCC=0).
    if is_cjk_ideograph(c as u32) {
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
            // The table stores fully-recursive decompositions, so the
            // singleton target is final — just look up its CCC.
            push_final_char(decomposed, output);
        }
        DecompResult::Expansion { offset, length } => {
            // The table stores fully-recursive decompositions, so each
            // expansion character is final — just look up CCC, no recursion.
            decode_expansion(offset, length, form, output);
        }
    }
}

/// Decompose a character using a pre-looked-up trie value.
///
/// The caller has already performed the trie lookup and knows the character
/// has a decomposition (HAS_DECOMPOSITION bit is set). This avoids a redundant
/// trie lookup in the hot path.
///
/// MUST NOT be called for ASCII, CJK ideographs, or Hangul syllables (they
/// are handled by fast paths before the trie lookup).
#[inline]
pub(crate) fn decompose_from_trie_value(
    c: char,
    trie_value: u32,
    output: &mut CccBuffer,
    form: DecompForm,
) {
    let (decomp, _ccc) = tables::decode_trie_value(trie_value, form);
    match decomp {
        DecompResult::None => {
            // Shouldn't happen if caller checked has_decomposition, but handle
            // gracefully: treat as self-mapping with its CCC.
            output.push(c, _ccc);
        }
        DecompResult::Singleton(decomposed) => {
            push_final_char(decomposed, output);
        }
        DecompResult::Expansion { offset, length } => {
            decode_expansion(offset, length, form, output);
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
