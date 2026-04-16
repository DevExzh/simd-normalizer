// src/compose.rs

//! Canonical composition engine.
//!
//! Provides pairwise character composition and the combining-sequence
//! composition algorithm described in UAX#15 Section 4.

use alloc::string::String;
use alloc::vec::Vec;

use crate::ccc::CharAndCcc;
use crate::hangul;
use crate::tables;

/// Try to compose two characters into one.
/// Checks Hangul first (algorithmic), then table lookup.
#[inline(always)]
pub(crate) fn compose(a: char, b: char) -> Option<char> {
    hangul::compose_hangul(a, b).or_else(|| tables::compose_pair(a, b))
}

/// Given a starter and a sorted sequence of combining characters (from CccBuffer),
/// perform canonical composition per UAX#15 Section 4.
///
/// The combining slice is assumed to already be sorted by CCC (canonical ordering).
/// A combining character is "blocked" from the starter when an intervening
/// character has CCC >= the candidate's CCC (or CCC == 0, which is a new starter).
///
/// Returns the (possibly recomposed) starter and any remaining characters
/// that could not be composed (in order).
pub(crate) fn compose_combining_sequence(
    starter: char,
    combining: &[CharAndCcc],
) -> (char, Vec<char>) {
    if combining.is_empty() {
        return (starter, Vec::new());
    }

    let mut current_starter = starter;
    let mut remaining: Vec<char> = Vec::new();
    // last_ccc tracks the CCC of the most recently *kept* (non-composed)
    // combining character. A combining character is "blocked" from the
    // starter when an intervening character has CCC >= this one's CCC.
    let mut last_ccc: Option<u8> = None;

    for entry in combining {
        let ch = entry.ch;
        let ch_ccc = entry.ccc;

        let blocked = match last_ccc {
            None => false,
            Some(prev_ccc) => prev_ccc >= ch_ccc,
        };

        if !blocked && let Some(composed) = compose(current_starter, ch) {
            current_starter = composed;
            // Do NOT update last_ccc: composed char disappears from sequence.
            continue;
        }

        // Either blocked or composition failed -- keep ch in output.
        remaining.push(ch);
        last_ccc = Some(ch_ccc);
    }

    (current_starter, remaining)
}

/// Like [`compose_combining_sequence`], but writes results directly to `out`,
/// avoiding the heap allocation for the remaining-characters `Vec`.
///
/// Uses a `u32` bitmask to track which combining entries were consumed by
/// composition, supporting up to 32 combining marks without allocation.
/// Sequences longer than 32 marks fall back to the allocating version.
#[inline]
pub(crate) fn compose_combining_sequence_into(
    starter: char,
    combining: &[CharAndCcc],
    out: &mut String,
) {
    if combining.is_empty() {
        out.push(starter);
        return;
    }

    // For sequences > 32, fall back to the allocating version.
    if combining.len() > 32 {
        let (composed, remaining) = compose_combining_sequence(starter, combining);
        out.push(composed);
        for ch in &remaining {
            out.push(*ch);
        }
        return;
    }

    let mut current_starter = starter;
    let mut last_ccc: Option<u8> = None;
    let mut composed_mask: u32 = 0;

    for (i, entry) in combining.iter().enumerate() {
        let blocked = match last_ccc {
            None => false,
            Some(prev_ccc) => prev_ccc >= entry.ccc,
        };

        if !blocked && let Some(composed) = compose(current_starter, entry.ch) {
            current_starter = composed;
            composed_mask |= 1u32 << i;
            continue;
        }

        last_ccc = Some(entry.ccc);
    }

    out.push(current_starter);
    for (i, entry) in combining.iter().enumerate() {
        if (composed_mask & (1u32 << i)) != 0 {
            continue;
        }
        out.push(entry.ch);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    // ---- compose() pairwise tests ----

    #[test]
    fn compose_e_acute() {
        assert_eq!(compose('e', '\u{0301}'), Some('\u{00E9}'));
    }

    #[test]
    fn compose_a_ring() {
        assert_eq!(compose('a', '\u{030A}'), Some('\u{00E5}'));
    }

    #[test]
    fn compose_no_composition() {
        assert_eq!(compose('a', 'b'), None);
    }

    #[test]
    fn compose_hangul_lv() {
        assert_eq!(compose('\u{1100}', '\u{1161}'), Some('\u{AC00}'));
    }

    #[test]
    fn compose_hangul_lvt() {
        assert_eq!(compose('\u{AC00}', '\u{11A8}'), Some('\u{AC01}'));
    }

    #[test]
    fn compose_hangul_lv_t_base_rejected() {
        assert_eq!(compose('\u{AC00}', '\u{11A7}'), None);
    }

    #[test]
    fn compose_hangul_wrong_pair() {
        assert_eq!(compose('\u{1161}', '\u{1100}'), None);
    }

    // ---- compose_combining_sequence() tests ----

    fn make_entry(ch: char, ccc: u8) -> CharAndCcc {
        CharAndCcc { ch, ccc }
    }

    #[test]
    fn compose_sequence_single_combining() {
        let combining = [make_entry('\u{0301}', 230)];
        let (starter, remaining) = compose_combining_sequence('e', &combining);
        assert_eq!(starter, '\u{00E9}');
        assert!(remaining.is_empty());
    }

    #[test]
    fn compose_sequence_blocked_same_ccc() {
        // a + tilde (CCC 230) + acute (CCC 230)
        // Acute is BLOCKED by tilde (same CCC), so only a+tilde composes.
        let combining = [
            make_entry('\u{0303}', 230), // tilde
            make_entry('\u{0301}', 230), // acute
        ];
        let (starter, remaining) = compose_combining_sequence('a', &combining);
        assert_eq!(starter, '\u{00E3}'); // a-tilde
        assert_eq!(remaining, vec!['\u{0301}']);
    }

    #[test]
    fn compose_sequence_not_blocked_different_ccc() {
        // o + cedilla (CCC 202) + acute (CCC 230)
        // Cedilla doesn't block acute (202 < 230).
        // o + cedilla: no composition. o + acute: composes to ó.
        let combining = [
            make_entry('\u{0327}', 202), // cedilla
            make_entry('\u{0301}', 230), // acute
        ];
        let (starter, remaining) = compose_combining_sequence('o', &combining);
        assert_eq!(starter, '\u{00F3}'); // o-acute
        assert_eq!(remaining, vec!['\u{0327}']);
    }

    #[test]
    fn compose_sequence_hangul_lvt() {
        // L + V + T -> LVT
        let combining = [
            make_entry('\u{1161}', 0), // V
            make_entry('\u{11A8}', 0), // T
        ];
        let (starter, remaining) = compose_combining_sequence('\u{1100}', &combining);
        assert_eq!(starter, '\u{AC01}'); // LVT GAG
        assert!(remaining.is_empty());
    }

    #[test]
    fn compose_sequence_nothing_composes() {
        let combining = [make_entry('\u{0308}', 230)];
        let (starter, remaining) = compose_combining_sequence('z', &combining);
        assert_eq!(starter, 'z');
        assert_eq!(remaining, vec!['\u{0308}']);
    }

    #[test]
    fn compose_sequence_empty() {
        let (starter, remaining) = compose_combining_sequence('A', &[]);
        assert_eq!(starter, 'A');
        assert!(remaining.is_empty());
    }
}
