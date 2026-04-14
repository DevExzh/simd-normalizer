//! Table access API -- trie lookup wrappers for decomposition, composition,
//! CCC, and quick-check data.

pub(crate) mod ccc;
pub(crate) mod composition;
pub(crate) mod decomposition;
pub(crate) mod qc;
pub(crate) mod trie;

use trie::CodePointTrie;

// ---------------------------------------------------------------------------
// Bit-field constants for packed decomposition trie values
// ---------------------------------------------------------------------------

/// First character of decomposition combines backwards.
#[allow(dead_code)]
const BACKWARD_COMBINING: u32 = 1 << 31;
/// Decomposition doesn't round-trip via NFC.
#[allow(dead_code)]
const NON_ROUND_TRIP: u32 = 1 << 30;
/// Code point needs decomposition (not a self-mapping).
const HAS_DECOMPOSITION: u32 = 1 << 29;
/// If set, DECOMP_INFO is an offset into the expansion table (not a singleton).
const IS_EXPANSION: u32 = 1 << 24;
/// Shift to extract the CCC byte from a decomposition trie value.
const CCC_SHIFT: u32 = 16;
/// Mask to isolate the CCC byte after shifting.
const CCC_MASK: u32 = 0xFF << CCC_SHIFT;
/// Mask for the 16-bit decomposition info field.
const DECOMP_INFO_MASK: u32 = 0xFFFF;

// ---------------------------------------------------------------------------
// DecompResult
// ---------------------------------------------------------------------------

/// Result of decoding a decomposition trie value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DecompResult {
    /// No decomposition (character maps to itself).
    None,
    /// Singleton BMP decomposition.
    Singleton(char),
    /// Expansion: offset and length into the relevant expansion table.
    Expansion { offset: usize, length: usize },
}

// ---------------------------------------------------------------------------
// Static trie constructors
// ---------------------------------------------------------------------------

/// Build the canonical decomposition trie from generated data.
#[inline]
pub(crate) fn canonical_trie() -> CodePointTrie {
    CodePointTrie {
        bmp_index: decomposition::CANONICAL_BMP_INDEX,
        data: decomposition::CANONICAL_TRIE_DATA,
        supp_index1: decomposition::CANONICAL_SUPP_INDEX1,
        supp_index2: decomposition::CANONICAL_SUPP_INDEX2,
        default_value: 0,
    }
}

/// Build the compatibility decomposition trie from generated data.
#[inline]
pub(crate) fn compat_trie() -> CodePointTrie {
    CodePointTrie {
        bmp_index: decomposition::COMPAT_BMP_INDEX,
        data: decomposition::COMPAT_TRIE_DATA,
        supp_index1: decomposition::COMPAT_SUPP_INDEX1,
        supp_index2: decomposition::COMPAT_SUPP_INDEX2,
        default_value: 0,
    }
}

/// Build the CCC trie from generated data.
#[inline]
pub(crate) fn ccc_trie() -> CodePointTrie {
    CodePointTrie {
        bmp_index: ccc::CCC_BMP_INDEX,
        data: ccc::CCC_TRIE_DATA,
        supp_index1: ccc::CCC_SUPP_INDEX1,
        supp_index2: ccc::CCC_SUPP_INDEX2,
        default_value: 0,
    }
}

/// Build the NFC quick-check trie.
#[inline]
pub(crate) fn nfc_qc_trie() -> CodePointTrie {
    CodePointTrie {
        bmp_index: qc::NFC_QC_BMP_INDEX,
        data: qc::NFC_QC_TRIE_DATA,
        supp_index1: qc::NFC_QC_SUPP_INDEX1,
        supp_index2: qc::NFC_QC_SUPP_INDEX2,
        default_value: 0,
    }
}

/// Build the NFD quick-check trie.
#[inline]
pub(crate) fn nfd_qc_trie() -> CodePointTrie {
    CodePointTrie {
        bmp_index: qc::NFD_QC_BMP_INDEX,
        data: qc::NFD_QC_TRIE_DATA,
        supp_index1: qc::NFD_QC_SUPP_INDEX1,
        supp_index2: qc::NFD_QC_SUPP_INDEX2,
        default_value: 0,
    }
}

/// Build the NFKC quick-check trie.
#[inline]
pub(crate) fn nfkc_qc_trie() -> CodePointTrie {
    CodePointTrie {
        bmp_index: qc::NFKC_QC_BMP_INDEX,
        data: qc::NFKC_QC_TRIE_DATA,
        supp_index1: qc::NFKC_QC_SUPP_INDEX1,
        supp_index2: qc::NFKC_QC_SUPP_INDEX2,
        default_value: 0,
    }
}

/// Build the NFKD quick-check trie.
#[inline]
pub(crate) fn nfkd_qc_trie() -> CodePointTrie {
    CodePointTrie {
        bmp_index: qc::NFKD_QC_BMP_INDEX,
        data: qc::NFKD_QC_TRIE_DATA,
        supp_index1: qc::NFKD_QC_SUPP_INDEX1,
        supp_index2: qc::NFKD_QC_SUPP_INDEX2,
        default_value: 0,
    }
}

// ---------------------------------------------------------------------------
// Decoding helpers
// ---------------------------------------------------------------------------

/// Extract CCC from a decomposition trie value.
#[inline]
pub(crate) fn ccc_from_trie_value(v: u32) -> u8 {
    ((v & CCC_MASK) >> CCC_SHIFT) as u8
}

/// Decode decomposition from a trie value.
///
/// For expansions, reads length from `expansion_table[offset]` and data
/// follows at `offset + 1`.
#[inline]
pub(crate) fn decode_decomp(trie_value: u32, expansion_table: &[u16]) -> DecompResult {
    if trie_value & HAS_DECOMPOSITION == 0 {
        return DecompResult::None;
    }
    let info = trie_value & DECOMP_INFO_MASK;
    if trie_value & IS_EXPANSION != 0 {
        // Expansion: info is offset into length-prefixed expansion table.
        let offset = info as usize;
        let length = expansion_table[offset] as usize;
        DecompResult::Expansion {
            offset: offset + 1,
            length,
        }
    } else {
        // Singleton BMP decomposition.
        // SAFETY: The table generator guarantees info is a valid BMP code point.
        debug_assert!(info <= 0xD7FF || (0xE000..=0xFFFF).contains(&info));
        let ch = unsafe { char::from_u32_unchecked(info) };
        DecompResult::Singleton(ch)
    }
}

/// Look up canonical decomposition for a character.
///
/// Returns `(decomp_result, ccc)` -- both extracted from the same trie lookup.
#[inline]
pub(crate) fn lookup_canonical_decomp(c: char) -> (DecompResult, u8) {
    let trie = canonical_trie();
    let v = trie.get(c as u32);
    let ccc = ccc_from_trie_value(v);
    let decomp = decode_decomp(v, decomposition::CANONICAL_EXPANSIONS);
    (decomp, ccc)
}

/// Look up compatibility decomposition for a character.
///
/// Returns `(decomp_result, ccc)` -- both extracted from the same trie lookup.
#[inline]
pub(crate) fn lookup_compat_decomp(c: char) -> (DecompResult, u8) {
    let trie = compat_trie();
    let v = trie.get(c as u32);
    let ccc = ccc_from_trie_value(v);
    let decomp = decode_decomp(v, decomposition::COMPAT_EXPANSIONS);
    (decomp, ccc)
}

/// Look up the Canonical Combining Class from the dedicated CCC trie.
#[inline]
pub(crate) fn lookup_ccc(c: char) -> u8 {
    let trie = ccc_trie();
    trie.get(c as u32) as u8
}

/// Compose a `(starter, combining)` pair.
///
/// Returns `Some(composed)` if the pair is canonically composable.
#[inline]
pub(crate) fn compose_pair(a: char, b: char) -> Option<char> {
    let key = ((a as u64) << 21) | (b as u64);
    let pairs = composition::COMPOSITION_PAIRS;
    let mut len = pairs.len();
    let mut base = 0usize;

    while len > 1 {
        let half = len / 2;
        // Branchless: if pairs[base + half].0 <= key, advance base.
        // The compiler should emit cmov for this pattern.
        base += (pairs[base + half].0 <= key) as usize * half;
        len -= half;
    }

    if base < pairs.len() && pairs[base].0 == key {
        // SAFETY: composition table only contains valid Unicode scalar values
        debug_assert!(pairs[base].1 <= 0x10FFFF && !(0xD800..=0xDFFF).contains(&(pairs[base].1)));
        Some(unsafe { char::from_u32_unchecked(pairs[base].1) })
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Quick-check lookups (0=Yes, 1=Maybe, 2=No)
// ---------------------------------------------------------------------------

/// NFC quick-check: 0=Yes, 1=Maybe, 2=No.
#[inline]
pub(crate) fn lookup_nfc_qc(c: char) -> u8 {
    nfc_qc_trie().get(c as u32) as u8
}

/// NFD quick-check: 0=Yes, 1=Maybe, 2=No.
#[inline]
pub(crate) fn lookup_nfd_qc(c: char) -> u8 {
    nfd_qc_trie().get(c as u32) as u8
}

/// NFKC quick-check: 0=Yes, 1=Maybe, 2=No.
#[inline]
pub(crate) fn lookup_nfkc_qc(c: char) -> u8 {
    nfkc_qc_trie().get(c as u32) as u8
}

/// NFKD quick-check: 0=Yes, 1=Maybe, 2=No.
#[inline]
pub(crate) fn lookup_nfkd_qc(c: char) -> u8 {
    nfkd_qc_trie().get(c as u32) as u8
}

// ---------------------------------------------------------------------------
// Expansion data accessors
// ---------------------------------------------------------------------------

/// Read expansion data from the canonical expansion table.
#[inline]
pub(crate) fn canonical_expansion_data(offset: usize, length: usize) -> &'static [u16] {
    &decomposition::CANONICAL_EXPANSIONS[offset..offset + length]
}

/// Read expansion data from the compatibility expansion table.
#[inline]
pub(crate) fn compat_expansion_data(offset: usize, length: usize) -> &'static [u16] {
    &decomposition::COMPAT_EXPANSIONS[offset..offset + length]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Synthetic unit tests (hand-crafted trie values)
    // -----------------------------------------------------------------------

    #[test]
    fn test_ccc_extraction() {
        // CCC = 0
        assert_eq!(ccc_from_trie_value(0), 0);
        // CCC = 230 (0xE6) at bits 23..16
        let v = 0xE6 << 16;
        assert_eq!(ccc_from_trie_value(v), 230);
        // CCC = 1
        let v = 0x01 << 16;
        assert_eq!(ccc_from_trie_value(v), 1);
        // CCC = 254
        let v = 0xFE << 16;
        assert_eq!(ccc_from_trie_value(v), 254);
    }

    #[test]
    fn test_decode_decomp_none() {
        // HAS_DECOMPOSITION not set -> None
        let dummy: [u16; 0] = [];
        assert_eq!(decode_decomp(0, &dummy), DecompResult::None);
        // Even with other bits set, if HAS_DECOMPOSITION is clear, it's None.
        assert_eq!(
            decode_decomp(BACKWARD_COMBINING | NON_ROUND_TRIP | 0x00FF_FFFF, &dummy),
            DecompResult::None
        );
    }

    #[test]
    fn test_decode_decomp_singleton() {
        // HAS_DECOMPOSITION set, IS_EXPANSION not set -> Singleton
        let v = HAS_DECOMPOSITION | 0x0041; // 'A'
        let dummy: [u16; 0] = [];
        assert_eq!(decode_decomp(v, &dummy), DecompResult::Singleton('A'));
    }

    #[test]
    fn test_decode_decomp_expansion() {
        // HAS_DECOMPOSITION | IS_EXPANSION, info = 3 (offset into expansion table)
        let v = HAS_DECOMPOSITION | IS_EXPANSION | 0x0003;
        // expansion_table[3] = 2 (length), followed by data at [4] and [5].
        let expansion_table: [u16; 6] = [0, 0, 0, 2, 0x0041, 0x0042];
        assert_eq!(
            decode_decomp(v, &expansion_table),
            DecompResult::Expansion {
                offset: 4,
                length: 2
            }
        );
    }

    #[test]
    fn test_ccc_from_trie_value_with_other_bits() {
        // CCC = 202, plus HAS_DECOMPOSITION and DECOMP_INFO bits
        let v = HAS_DECOMPOSITION | (202u32 << 16) | 0x1234;
        assert_eq!(ccc_from_trie_value(v), 202);
    }

    // -----------------------------------------------------------------------
    // Integration tests against real generated data
    // -----------------------------------------------------------------------

    #[test]
    fn test_compose_pair_a_grave() {
        // 'A' (U+0041) + U+0300 (COMBINING GRAVE ACCENT) -> U+00C0 (LATIN CAPITAL LETTER A WITH GRAVE)
        let result = compose_pair('A', '\u{0300}');
        assert_eq!(result, Some('\u{00C0}'));
    }

    #[test]
    fn test_compose_pair_e_acute() {
        // 'E' (U+0045) + U+0301 (COMBINING ACUTE ACCENT) -> U+00C9 (LATIN CAPITAL LETTER E WITH ACUTE)
        let result = compose_pair('E', '\u{0301}');
        assert_eq!(result, Some('\u{00C9}'));
    }

    #[test]
    fn test_compose_pair_nonexistent() {
        // 'Z' + U+0300 -- check if it exists; if it doesn't, should return None.
        // (In Unicode, Z + grave does not compose to a precomposed character.)
        let result = compose_pair('Z', '\u{0300}');
        assert_eq!(result, Option::None);
    }

    #[test]
    fn test_compose_pair_non_composable() {
        // Two random ASCII characters should not compose.
        assert_eq!(compose_pair('x', 'y'), Option::None);
    }

    #[test]
    fn test_lookup_ccc_grave_accent() {
        // U+0300 COMBINING GRAVE ACCENT has CCC = 230.
        assert_eq!(lookup_ccc('\u{0300}'), 230);
    }

    #[test]
    fn test_lookup_ccc_cedilla() {
        // U+0327 COMBINING CEDILLA has CCC = 202.
        assert_eq!(lookup_ccc('\u{0327}'), 202);
    }

    #[test]
    fn test_lookup_ccc_ascii() {
        // ASCII 'A' has CCC = 0.
        assert_eq!(lookup_ccc('A'), 0);
    }

    #[test]
    fn test_canonical_decomp_ascii() {
        // ASCII 'A' should have no canonical decomposition.
        let (decomp, ccc) = lookup_canonical_decomp('A');
        assert_eq!(decomp, DecompResult::None);
        assert_eq!(ccc, 0);
    }

    #[test]
    fn test_canonical_decomp_a_grave() {
        // U+00C0 (LATIN CAPITAL LETTER A WITH GRAVE) decomposes canonically
        // to 'A' (U+0041) + U+0300.
        let (decomp, _ccc) = lookup_canonical_decomp('\u{00C0}');
        match decomp {
            DecompResult::Expansion { offset, length } => {
                let data = canonical_expansion_data(offset, length);
                // Should be [0x0041, 0x0300]
                assert_eq!(data.len(), 2);
                assert_eq!(data[0], 0x0041); // 'A'
                assert_eq!(data[1], 0x0300); // combining grave accent
            }
            DecompResult::Singleton(ch) => {
                // Some generators produce singleton for single-char decomp.
                // But U+00C0 decomposes to two characters, so this shouldn't happen.
                panic!("Expected Expansion for U+00C0, got Singleton({ch:?})");
            }
            DecompResult::None => {
                panic!("Expected decomposition for U+00C0, got None");
            }
        }
    }

    #[test]
    fn test_nfc_qc_ascii() {
        // ASCII characters are NFC quick-check Yes (0).
        assert_eq!(lookup_nfc_qc('A'), 0);
        assert_eq!(lookup_nfc_qc('z'), 0);
    }

    #[test]
    fn test_nfd_qc_ascii() {
        // ASCII characters are NFD quick-check Yes (0).
        assert_eq!(lookup_nfd_qc('A'), 0);
    }

    #[test]
    fn test_nfd_qc_precomposed() {
        // U+00C0 (A-grave) is NFD_QC = No (2) -- it has a canonical decomposition.
        let v = lookup_nfd_qc('\u{00C0}');
        assert_eq!(v, 2);
    }

    #[test]
    fn test_nfc_qc_combining_mark() {
        // U+0300 (COMBINING GRAVE ACCENT) -- not quick-check Yes (either Maybe or No
        // depending on generator encoding).
        let v = lookup_nfc_qc('\u{0300}');
        assert_ne!(v, 0, "U+0300 should not be NFC_QC=Yes");
    }

    #[test]
    fn test_nfkd_qc_ascii() {
        // ASCII is NFKD_QC Yes.
        assert_eq!(lookup_nfkd_qc('a'), 0);
    }

    #[test]
    fn test_nfkc_qc_ascii() {
        // ASCII is NFKC_QC Yes.
        assert_eq!(lookup_nfkc_qc('a'), 0);
    }

    #[test]
    fn test_trie_constructors_dont_panic() {
        // Smoke test: constructing each trie should not panic.
        let _ = canonical_trie();
        let _ = compat_trie();
        let _ = ccc_trie();
        let _ = nfc_qc_trie();
        let _ = nfd_qc_trie();
        let _ = nfkc_qc_trie();
        let _ = nfkd_qc_trie();
    }

    #[test]
    fn test_backward_combining_and_non_round_trip_bits() {
        // Verify the bit constants don't overlap and are correctly positioned.
        assert_eq!(BACKWARD_COMBINING & NON_ROUND_TRIP, 0);
        assert_eq!(BACKWARD_COMBINING & HAS_DECOMPOSITION, 0);
        assert_eq!(NON_ROUND_TRIP & HAS_DECOMPOSITION, 0);
        assert_eq!(HAS_DECOMPOSITION & IS_EXPANSION, 0);
        assert_eq!(IS_EXPANSION & CCC_MASK, 0);
        assert_eq!(CCC_MASK & DECOMP_INFO_MASK, 0);
    }
}
