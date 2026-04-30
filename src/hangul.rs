//! Algorithmic Hangul Syllable decomposition and composition.
//!
//! Hangul Syllables (U+AC00..U+D7A3) are decomposed/composed using Jamo
//! arithmetic per Unicode Standard Section 3.12, not via trie tables.

/// Syllable base.
pub(crate) const S_BASE: u32 = 0xAC00;
/// Leading consonant (L) jamo base.
pub(crate) const L_BASE: u32 = 0x1100;
/// Vowel (V) jamo base.
pub(crate) const V_BASE: u32 = 0x1161;
/// Trailing consonant (T) jamo base.
/// Note: T_BASE itself (0x11A7) is *not* a valid trailing consonant.
pub(crate) const T_BASE: u32 = 0x11A7;
/// Number of leading consonant jamos.
pub(crate) const L_COUNT: u32 = 19;
/// Number of vowel jamos.
pub(crate) const V_COUNT: u32 = 21;
/// Number of trailing consonant jamos (including "no trailing" = index 0).
pub(crate) const T_COUNT: u32 = 28;
/// V_COUNT * T_COUNT.
pub(crate) const N_COUNT: u32 = V_COUNT * T_COUNT; // 588
/// Total number of Hangul Syllables.
pub(crate) const S_COUNT: u32 = L_COUNT * N_COUNT; // 11172

/// Check if a character is a Hangul Syllable (U+AC00..U+D7A3).
#[inline]
pub(crate) fn is_hangul_syllable(c: char) -> bool {
    let cp = c as u32;
    (S_BASE..S_BASE + S_COUNT).contains(&cp)
}

/// Check if a character is a Hangul L (leading consonant) jamo.
#[allow(dead_code)]
#[inline]
pub(crate) fn is_hangul_lpart(c: char) -> bool {
    let cp = c as u32;
    (L_BASE..L_BASE + L_COUNT).contains(&cp)
}

/// Check if a character is a Hangul V (vowel) jamo.
#[allow(dead_code)]
#[inline]
pub(crate) fn is_hangul_vpart(c: char) -> bool {
    let cp = c as u32;
    (V_BASE..V_BASE + V_COUNT).contains(&cp)
}

/// Check if a character is a Hangul T (trailing consonant) jamo.
/// Note: T_BASE (U+11A7) itself is NOT a valid trailing consonant.
#[allow(dead_code)]
#[inline]
pub(crate) fn is_hangul_tpart(c: char) -> bool {
    let cp = c as u32;
    cp > T_BASE && cp < T_BASE + T_COUNT
}

/// Decompose a Hangul Syllable into its constituent jamos.
/// Returns `(L, V, None)` for LV syllables or `(L, V, Some(T))` for LVT.
#[inline]
pub(crate) fn decompose_hangul(c: char) -> (char, char, Option<char>) {
    debug_assert!(is_hangul_syllable(c));
    let s_index = c as u32 - S_BASE;
    let l_index = s_index / N_COUNT;
    let v_index = (s_index % N_COUNT) / T_COUNT;
    let t_index = s_index % T_COUNT;
    let l = unsafe { char::from_u32_unchecked(L_BASE + l_index) };
    let v = unsafe { char::from_u32_unchecked(V_BASE + v_index) };
    let t = if t_index > 0 {
        Some(unsafe { char::from_u32_unchecked(T_BASE + t_index) })
    } else {
        None
    };
    (l, v, t)
}

/// Decompose a Hangul Syllable directly into UTF-8 bytes appended to `out`.
///
/// Writes 6 bytes (LV) or 9 bytes (LVT) in a single `extend_from_slice` after
/// one capacity-reserve, instead of calling `String::push` three times. The
/// per-`push` capacity check + per-char UTF-8 encode dominates the syllable
/// cost on the NFD/NFKD Hangul path; consolidating into one reserve+memcpy is
/// a measurable win on dense Hangul (`gen_hangul`).
///
/// All Hangul jamos are 3-byte UTF-8 in the U+1100..U+11FF block, all
/// starting with 0xE1. We construct the byte sequence inline from the L/V/T
/// indices without going through `char::from_u32_unchecked` + `encode_utf8`.
#[inline]
pub(crate) fn push_decomposed_hangul(c: char, out: &mut alloc::string::String) {
    debug_assert!(is_hangul_syllable(c));
    let s_index = c as u32 - S_BASE;
    let l_index = s_index / N_COUNT;
    let v_index = (s_index % N_COUNT) / T_COUNT;
    let t_index = s_index % T_COUNT;

    // L: cp = 0x1100 + l_index (l_index in 0..19, all in 0x1100..0x1112).
    //    UTF-8 = [0xE1, 0x84, 0x80 | l_index].
    // V: cp = 0x1161 + v_index (v_index in 0..21, all in 0x1161..0x1175).
    //    UTF-8 = [0xE1, 0x85, 0xA1 | v_index] (since cp & 0x3F = 0x21 + v_index, no overflow).
    //    Verified: v_index <= 20, so 0xA1 + 20 = 0xB5 < 0xC0.
    // T: cp = 0x11A7 + t_index (t_index in 1..28, all in 0x11A8..0x11C3).
    //    For cp in 0x11A8..0x11C0: byte1=0x86, byte2 = 0x80 | (cp & 0x3F) = 0xA8..0xBF.
    //    For cp in 0x11C0..0x11C3: byte1=0x87, byte2 = 0x80 | (cp & 0x3F) = 0x80..0x83.

    let l_b2 = 0x80 | (l_index as u8);
    let v_b2 = 0xA1 + (v_index as u8);

    if t_index == 0 {
        // 6-byte LV decomposition.
        let bytes = [0xE1u8, 0x84, l_b2, 0xE1, 0x85, v_b2];
        out.push_str(unsafe { core::str::from_utf8_unchecked(&bytes) });
    } else {
        // 9-byte LVT decomposition.
        let t_cp = T_BASE + t_index;
        let t_b1 = 0x80 | ((t_cp >> 6) & 0x3F) as u8; // 0x86 or 0x87
        let t_b2 = 0x80 | (t_cp & 0x3F) as u8;
        let bytes = [0xE1u8, 0x84, l_b2, 0xE1, 0x85, v_b2, 0xE1, t_b1, t_b2];
        out.push_str(unsafe { core::str::from_utf8_unchecked(&bytes) });
    }
}

/// Attempt to compose a Hangul pair.
/// L + V -> LV Syllable, LV Syllable + T -> LVT Syllable.
/// Returns `None` if pair does not form a Hangul composition.
#[inline]
pub(crate) fn compose_hangul(a: char, b: char) -> Option<char> {
    let a_cp = a as u32;
    let b_cp = b as u32;
    // Case 1: L + V -> LV
    let l_index = a_cp.wrapping_sub(L_BASE);
    if l_index < L_COUNT {
        let v_index = b_cp.wrapping_sub(V_BASE);
        if v_index < V_COUNT {
            let lv = S_BASE + l_index * N_COUNT + v_index * T_COUNT;
            return Some(unsafe { char::from_u32_unchecked(lv) });
        }
        return None;
    }
    // Case 2: LV + T -> LVT
    let s_index = a_cp.wrapping_sub(S_BASE);
    if s_index < S_COUNT && s_index.is_multiple_of(T_COUNT) {
        let t_index = b_cp.wrapping_sub(T_BASE);
        if t_index > 0 && t_index < T_COUNT {
            return Some(unsafe { char::from_u32_unchecked(a_cp + t_index) });
        }
    }
    None
}

/// Return the decomposition length: 2 for LV, 3 for LVT.
#[allow(dead_code)]
#[inline]
pub(crate) fn hangul_decomposition_length(c: char) -> usize {
    debug_assert!(is_hangul_syllable(c));
    let s_index = c as u32 - S_BASE;
    if s_index.is_multiple_of(T_COUNT) {
        2
    } else {
        3
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_hangul_syllable_first() {
        assert!(is_hangul_syllable('\u{AC00}'));
    }

    #[test]
    fn test_is_hangul_syllable_last() {
        assert!(is_hangul_syllable('\u{D7A3}'));
    }

    #[test]
    fn test_is_hangul_syllable_before_range() {
        assert!(!is_hangul_syllable('\u{ABFF}'));
    }

    #[test]
    fn test_is_hangul_syllable_after_range() {
        assert!(!is_hangul_syllable('\u{D7A4}'));
    }

    #[test]
    fn test_is_hangul_syllable_ascii() {
        assert!(!is_hangul_syllable('A'));
    }

    #[test]
    fn test_is_hangul_lpart_first() {
        assert!(is_hangul_lpart('\u{1100}'));
    }

    #[test]
    fn test_is_hangul_lpart_last() {
        assert!(is_hangul_lpart('\u{1112}'));
    }

    #[test]
    fn test_is_hangul_lpart_before() {
        assert!(!is_hangul_lpart('\u{10FF}'));
    }

    #[test]
    fn test_is_hangul_lpart_after() {
        assert!(!is_hangul_lpart('\u{1113}'));
    }

    #[test]
    fn test_is_hangul_vpart_first() {
        assert!(is_hangul_vpart('\u{1161}'));
    }

    #[test]
    fn test_is_hangul_vpart_last() {
        assert!(is_hangul_vpart('\u{1175}'));
    }

    #[test]
    fn test_is_hangul_vpart_before() {
        assert!(!is_hangul_vpart('\u{1160}'));
    }

    #[test]
    fn test_is_hangul_vpart_after() {
        assert!(!is_hangul_vpart('\u{1176}'));
    }

    #[test]
    fn test_is_hangul_tpart_first() {
        assert!(is_hangul_tpart('\u{11A8}'));
    }

    #[test]
    fn test_is_hangul_tpart_last() {
        assert!(is_hangul_tpart('\u{11C2}'));
    }

    #[test]
    fn test_is_hangul_tpart_excludes_t_base() {
        assert!(!is_hangul_tpart('\u{11A7}'));
    }

    #[test]
    fn test_is_hangul_tpart_before() {
        assert!(!is_hangul_tpart('\u{11A6}'));
    }

    #[test]
    fn test_is_hangul_tpart_after() {
        assert!(!is_hangul_tpart('\u{11C3}'));
    }

    #[test]
    fn test_decompose_hangul_lv_first() {
        let (l, v, t) = decompose_hangul('\u{AC00}');
        assert_eq!(l, '\u{1100}');
        assert_eq!(v, '\u{1161}');
        assert_eq!(t, None);
    }

    #[test]
    fn test_decompose_hangul_lv_second_l() {
        let (l, v, t) = decompose_hangul('\u{B098}');
        assert_eq!(l, '\u{1102}');
        assert_eq!(v, '\u{1161}');
        assert_eq!(t, None);
    }

    #[test]
    fn test_decompose_hangul_lvt_first_t() {
        let (l, v, t) = decompose_hangul('\u{AC01}');
        assert_eq!(l, '\u{1100}');
        assert_eq!(v, '\u{1161}');
        assert_eq!(t, Some('\u{11A8}'));
    }

    #[test]
    fn test_decompose_hangul_lvt_last_syllable() {
        let (l, v, t) = decompose_hangul('\u{D7A3}');
        assert_eq!(l, '\u{1112}');
        assert_eq!(v, '\u{1175}');
        assert_eq!(t, Some('\u{11C2}'));
    }

    #[test]
    fn test_compose_hangul_l_v_to_lv() {
        assert_eq!(compose_hangul('\u{1100}', '\u{1161}'), Some('\u{AC00}'));
    }

    #[test]
    fn test_compose_hangul_l_v_last() {
        assert_eq!(compose_hangul('\u{1112}', '\u{1175}'), Some('\u{D788}'));
    }

    #[test]
    fn test_compose_hangul_lv_t_to_lvt() {
        assert_eq!(compose_hangul('\u{AC00}', '\u{11A8}'), Some('\u{AC01}'));
    }

    #[test]
    fn test_compose_hangul_lv_t_last() {
        assert_eq!(compose_hangul('\u{D788}', '\u{11C2}'), Some('\u{D7A3}'));
    }

    #[test]
    fn test_compose_hangul_non_hangul_pair() {
        assert_eq!(compose_hangul('A', 'B'), None);
    }

    #[test]
    fn test_compose_hangul_l_with_non_v() {
        assert_eq!(compose_hangul('\u{1100}', 'A'), None);
    }

    #[test]
    fn test_compose_hangul_lv_with_t_base_rejected() {
        assert_eq!(compose_hangul('\u{AC00}', '\u{11A7}'), None);
    }

    #[test]
    fn test_compose_hangul_lvt_with_t_rejected() {
        assert_eq!(compose_hangul('\u{AC01}', '\u{11A8}'), None);
    }

    #[test]
    fn test_compose_hangul_v_with_t_rejected() {
        assert_eq!(compose_hangul('\u{1161}', '\u{11A8}'), None);
    }

    #[test]
    fn test_hangul_decomposition_length_lv() {
        assert_eq!(hangul_decomposition_length('\u{AC00}'), 2);
    }

    #[test]
    fn test_hangul_decomposition_length_lvt() {
        assert_eq!(hangul_decomposition_length('\u{AC01}'), 3);
    }

    #[test]
    fn test_hangul_decomposition_length_last() {
        assert_eq!(hangul_decomposition_length('\u{D7A3}'), 3);
    }

    #[test]
    fn test_all_lv_syllables_round_trip() {
        for l in 0..L_COUNT {
            for v in 0..V_COUNT {
                let syllable_cp = S_BASE + l * N_COUNT + v * T_COUNT;
                let syllable = char::from_u32(syllable_cp).unwrap();
                let (dl, dv, dt) = decompose_hangul(syllable);
                assert_eq!(dt, None);
                let recomposed = compose_hangul(dl, dv).unwrap();
                assert_eq!(recomposed, syllable);
            }
        }
    }

    #[test]
    fn test_sample_lvt_syllables_round_trip() {
        for l in [0u32, 5, 10, 18] {
            for v in [0u32, 10, 20] {
                for t in 1..T_COUNT {
                    let syllable_cp = S_BASE + l * N_COUNT + v * T_COUNT + t;
                    let syllable = char::from_u32(syllable_cp).unwrap();
                    let (dl, dv, dt) = decompose_hangul(syllable);
                    assert!(dt.is_some());
                    let lv = compose_hangul(dl, dv).unwrap();
                    let recomposed = compose_hangul(lv, dt.unwrap()).unwrap();
                    assert_eq!(recomposed, syllable);
                }
            }
        }
    }

    #[test]
    fn test_decomposition_length_matches_decompose_output() {
        for cp in (S_BASE..S_BASE + S_COUNT).step_by(37) {
            let c = char::from_u32(cp).unwrap();
            let (_, _, t) = decompose_hangul(c);
            let expected_len = if t.is_some() { 3 } else { 2 };
            assert_eq!(hangul_decomposition_length(c), expected_len);
        }
    }
}
