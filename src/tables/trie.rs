//! Two-level CodePointTrie optimized for BMP fast-path and sequential access.
//!
//! BMP lookup: `data[bmp_index[cp >> 5] + (cp & 0x1F)]` -- 2 array accesses, no branching.
//! Supplementary lookup: 3-level index hierarchy, cold path.

/// Shift for BMP index blocks (32 entries per block).
const BMP_SHIFT: u32 = 5;
/// Mask for the offset within a BMP block.
const BMP_MASK: u32 = (1 << BMP_SHIFT) - 1; // 0x1F

/// Shift for supplementary index level 1 (top-level partitioning).
const SUPP_SHIFT_1: u32 = 11;
/// Shift for supplementary index level 2 (mid-level partitioning).
const SUPP_SHIFT_2: u32 = 5;
/// Mask for supplementary level-2 offset.
const SUPP_MASK_2: u32 = (1 << (SUPP_SHIFT_1 - SUPP_SHIFT_2)) - 1; // 0x3F
/// Mask for supplementary data block offset.
const SUPP_MASK_DATA: u32 = (1 << SUPP_SHIFT_2) - 1; // 0x1F

/// A two-level code point trie with BMP fast-path and supplementary 3-level index.
///
/// All data is `&'static` -- the trie is zero-cost to construct and copy.
#[derive(Clone, Copy)]
pub(crate) struct CodePointTrie {
    /// Block pointers for BMP (U+0000..U+FFFF).
    /// 2048 entries (65536 >> 5), each pointing into `data`.
    pub(crate) bmp_index: &'static [u16],
    /// Trie values -- both BMP and supplementary data blocks live here.
    pub(crate) data: &'static [u32],
    /// Level-1 index for supplementary code points (U+10000..U+10FFFF).
    pub(crate) supp_index1: &'static [u16],
    /// Level-2 index for supplementary code points.
    pub(crate) supp_index2: &'static [u16],
    /// Default value returned for unmapped / out-of-range code points.
    pub(crate) default_value: u32,
}

impl CodePointTrie {
    /// Look up the trie value for a code point.
    #[inline]
    pub(crate) fn get(&self, cp: u32) -> u32 {
        if cp < 0x10000 {
            self.get_bmp(cp)
        } else if cp <= 0x10FFFF {
            self.get_supplementary(cp)
        } else {
            self.default_value
        }
    }

    /// BMP-only lookup path. Two array accesses, no branching.
    #[inline(always)]
    fn get_bmp(&self, cp: u32) -> u32 {
        debug_assert!(cp < 0x10000);
        let block_idx = (cp >> BMP_SHIFT) as usize;
        let offset = (cp & BMP_MASK) as usize;
        let base = self.bmp_index[block_idx] as usize;
        self.data[base + offset]
    }

    /// Supplementary lookup path (U+10000..U+10FFFF).
    #[cold]
    #[inline(never)]
    fn get_supplementary(&self, cp: u32) -> u32 {
        debug_assert!((0x10000..=0x10FFFF).contains(&cp));
        let adjusted = cp - 0x10000;

        let i1 = (adjusted >> SUPP_SHIFT_1) as usize;
        let l1_entry = match self.supp_index1.get(i1) {
            Some(&v) => v as usize,
            None => return self.default_value,
        };

        let i2_offset = ((adjusted >> SUPP_SHIFT_2) & SUPP_MASK_2) as usize;
        let l2_entry = match self.supp_index2.get(l1_entry + i2_offset) {
            Some(&v) => v as usize,
            None => return self.default_value,
        };

        let data_offset = (adjusted & SUPP_MASK_DATA) as usize;
        match self.data.get(l2_entry + data_offset) {
            Some(&v) => v,
            None => self.default_value,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_BMP_INDEX: [u16; 2048] = {
        let mut arr = [128u16; 2048]; // default block at data[128]
        arr[0] = 0; // U+0000..U+001F -> data[0]
        arr[1] = 32; // U+0020..U+003F -> data[32]
        arr[2] = 64; // U+0040..U+005F -> data[64]
        arr[3] = 96; // U+0060..U+007F -> data[96]
        arr[0x270] = 160; // U+4E00..U+4E1F -> data[160]
        arr
    };

    static TEST_DATA: [u32; 224] = {
        let mut arr = [0u32; 224];
        let mut i = 0u32;
        while i < 128 {
            arr[i as usize] = i;
            i += 1;
        }
        // CJK block at data[160..192]
        let mut j = 0u32;
        while j < 32 {
            arr[160 + j as usize] = 0xC000 + j;
            j += 1;
        }
        // Supplementary block at data[192..224]
        let mut k = 0u32;
        while k < 32 {
            arr[192 + k as usize] = 0xE000 + k;
            k += 1;
        }
        arr
    };

    static TEST_SUPP_INDEX1: [u16; 528] = {
        let mut arr = [64u16; 528]; // null block in supp_index2
        arr[0] = 0; // first L1 entry -> supp_index2[0]
        arr
    };

    static TEST_SUPP_INDEX2: [u16; 128] = {
        let mut arr = [128u16; 128]; // default data block
        arr[0] = 192; // -> data[192] (supplementary test block)
        arr
    };

    fn test_trie() -> CodePointTrie {
        CodePointTrie {
            bmp_index: &TEST_BMP_INDEX,
            data: &TEST_DATA,
            supp_index1: &TEST_SUPP_INDEX1,
            supp_index2: &TEST_SUPP_INDEX2,
            default_value: 0,
        }
    }

    #[test]
    fn test_ascii_lookup() {
        let trie = test_trie();
        assert_eq!(trie.get(0x00), 0x00);
        assert_eq!(trie.get(0x41), 0x41); // 'A'
        assert_eq!(trie.get(0x61), 0x61); // 'a'
        assert_eq!(trie.get(0x7F), 0x7F);
    }

    #[test]
    fn test_bmp_cjk_lookup() {
        let trie = test_trie();
        assert_eq!(trie.get(0x4E00), 0xC000);
        assert_eq!(trie.get(0x4E01), 0xC001);
        assert_eq!(trie.get(0x4E1F), 0xC01F);
    }

    #[test]
    fn test_supplementary_lookup() {
        let trie = test_trie();
        assert_eq!(trie.get(0x10000), 0xE000);
        assert_eq!(trie.get(0x10001), 0xE001);
        assert_eq!(trie.get(0x1001F), 0xE01F);
    }

    #[test]
    fn test_unmapped_returns_default() {
        let trie = test_trie();
        assert_eq!(trie.get(0x0100), 0);
        assert_eq!(trie.get(0x100000), 0);
        assert_eq!(trie.get(0x110000), 0);
        assert_eq!(trie.get(0xFFFFFFFF), 0);
    }

    #[test]
    fn test_bmp_boundary() {
        let trie = test_trie();
        assert_eq!(trie.get(0xFFFF), 0);
        assert_eq!(trie.get(0x10000), 0xE000);
    }

    #[test]
    fn test_all_ascii_round_trip() {
        let trie = test_trie();
        for cp in 0u32..=0x7F {
            assert_eq!(trie.get(cp), cp, "mismatch at U+{cp:04X}");
        }
    }

    #[test]
    fn test_supplementary_end_of_range() {
        let trie = test_trie();
        assert_eq!(trie.get(0x10FFFF), 0);
    }

    #[test]
    fn test_get_is_consistent_with_get_bmp() {
        let trie = test_trie();
        for cp in (0u32..0x10000).step_by(997) {
            assert_eq!(trie.get(cp), trie.get_bmp(cp), "mismatch at U+{cp:04X}");
        }
    }
}
