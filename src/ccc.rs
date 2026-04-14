//! Canonical Combining Class lookup and canonical ordering sort.
//!
//! The canonical ordering algorithm requires that combining marks are sorted
//! by their CCC value, with the sort being *stable* (preserving original
//! order among marks with the same CCC).

use alloc::vec::Vec;

use crate::tables;

/// A character paired with its Canonical Combining Class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CharAndCcc {
    pub(crate) ch: char,
    pub(crate) ccc: u8,
}

/// Inline capacity for the common case.
const INLINE_CAP: usize = 4;

/// Buffer for collecting combining characters before canonical ordering sort.
///
/// Uses inline storage for the common case (most combining sequences have
/// at most 4 marks). Falls back to heap allocation for longer sequences.
pub(crate) struct CccBuffer {
    inline: [CharAndCcc; INLINE_CAP],
    len: usize,
    overflow: Option<Vec<CharAndCcc>>,
}

impl CccBuffer {
    /// Create a new, empty buffer.
    #[inline]
    pub(crate) fn new() -> Self {
        CccBuffer {
            inline: [CharAndCcc { ch: '\0', ccc: 0 }; INLINE_CAP],
            len: 0,
            overflow: None,
        }
    }

    /// Push a character with its CCC into the buffer.
    #[inline]
    pub(crate) fn push(&mut self, ch: char, ccc: u8) {
        let entry = CharAndCcc { ch, ccc };
        if let Some(ref mut vec) = self.overflow {
            vec.push(entry);
            self.len = vec.len();
        } else if self.len < INLINE_CAP {
            self.inline[self.len] = entry;
            self.len += 1;
        } else {
            let mut vec = Vec::with_capacity(INLINE_CAP * 2);
            vec.extend_from_slice(&self.inline[..INLINE_CAP]);
            vec.push(entry);
            self.len = vec.len();
            self.overflow = Some(vec);
        }
    }

    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[allow(dead_code)]
    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.len
    }

    /// Sort entries by CCC (stable sort), then return an iterator that drains
    /// all elements in sorted order. After draining, buffer is empty and reusable.
    pub(crate) fn sort_and_drain(&mut self) -> SortedDrain<'_> {
        if let Some(ref mut vec) = self.overflow {
            vec.sort_by_key(|e| e.ccc);
        } else {
            insertion_sort_by_ccc(&mut self.inline[..self.len]);
        }
        SortedDrain { buf: self, pos: 0 }
    }

    /// Clear the buffer for reuse. Overflow Vec capacity is preserved.
    #[inline]
    pub(crate) fn clear(&mut self) {
        if let Some(ref mut vec) = self.overflow {
            vec.clear();
        }
        self.len = 0;
    }

    /// Access elements as a slice.
    pub(crate) fn as_slice(&self) -> &[CharAndCcc] {
        if let Some(ref vec) = self.overflow {
            &vec[..]
        } else {
            &self.inline[..self.len]
        }
    }
}

/// Draining iterator over a CccBuffer after sorting.
pub(crate) struct SortedDrain<'a> {
    buf: &'a mut CccBuffer,
    pos: usize,
}

impl Iterator for SortedDrain<'_> {
    type Item = CharAndCcc;

    #[inline]
    fn next(&mut self) -> Option<CharAndCcc> {
        if self.pos >= self.buf.len {
            self.buf.clear();
            return None;
        }
        let entry = if let Some(ref vec) = self.buf.overflow {
            vec[self.pos]
        } else {
            self.buf.inline[self.pos]
        };
        self.pos += 1;
        if self.pos >= self.buf.len {
            self.buf.clear();
        }
        Some(entry)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.buf.len.saturating_sub(self.pos);
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for SortedDrain<'_> {}

/// Stable insertion sort by CCC. Optimal for small arrays (n <= ~8).
fn insertion_sort_by_ccc(slice: &mut [CharAndCcc]) {
    for i in 1..slice.len() {
        let key = slice[i];
        let mut j = i;
        while j > 0 && slice[j - 1].ccc > key.ccc {
            slice[j] = slice[j - 1];
            j -= 1;
        }
        slice[j] = key;
    }
}

/// Look up the Canonical Combining Class for a character.
#[inline]
pub(crate) fn canonical_combining_class(c: char) -> u8 {
    tables::lookup_ccc(c)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;

    #[test]
    fn test_empty_buffer() {
        let buf = CccBuffer::new();
        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn test_push_and_len() {
        let mut buf = CccBuffer::new();
        buf.push('a', 0);
        assert_eq!(buf.len(), 1);
        assert!(!buf.is_empty());
        buf.push('\u{0301}', 230);
        assert_eq!(buf.len(), 2);
    }

    #[test]
    fn test_inline_sort_by_ccc() {
        let mut buf = CccBuffer::new();
        buf.push('\u{0327}', 202);
        buf.push('\u{0301}', 230);
        buf.push('\u{0308}', 230);
        buf.push('\u{0323}', 220);

        let sorted: Vec<CharAndCcc> = buf.sort_and_drain().collect();
        assert_eq!(sorted.len(), 4);
        assert_eq!(sorted[0].ccc, 202);
        assert_eq!(sorted[0].ch, '\u{0327}');
        assert_eq!(sorted[1].ccc, 220);
        assert_eq!(sorted[1].ch, '\u{0323}');
        // Stability: U+0301 before U+0308 (both CCC 230)
        assert_eq!(sorted[2].ch, '\u{0301}');
        assert_eq!(sorted[3].ch, '\u{0308}');
        assert!(buf.is_empty());
    }

    #[test]
    fn test_stability_same_ccc() {
        let mut buf = CccBuffer::new();
        buf.push('A', 230);
        buf.push('B', 230);
        buf.push('C', 230);
        let sorted: Vec<CharAndCcc> = buf.sort_and_drain().collect();
        assert_eq!(sorted[0].ch, 'A');
        assert_eq!(sorted[1].ch, 'B');
        assert_eq!(sorted[2].ch, 'C');
    }

    #[test]
    fn test_overflow_to_heap() {
        let mut buf = CccBuffer::new();
        buf.push('a', 0);
        buf.push('\u{0301}', 230);
        buf.push('\u{0308}', 230);
        buf.push('\u{0323}', 220);
        buf.push('\u{0327}', 202); // 5th, triggers overflow
        assert_eq!(buf.len(), 5);
        assert!(buf.overflow.is_some());
        let sorted: Vec<CharAndCcc> = buf.sort_and_drain().collect();
        assert_eq!(sorted.len(), 5);
        assert_eq!(sorted[0].ccc, 0);
        assert_eq!(sorted[1].ccc, 202);
        assert_eq!(sorted[2].ccc, 220);
        assert_eq!(sorted[3].ccc, 230);
        assert_eq!(sorted[4].ccc, 230);
        assert!(buf.is_empty());
    }

    #[test]
    fn test_overflow_stability() {
        let mut buf = CccBuffer::new();
        for ch in ['A', 'B', 'C', 'D', 'E', 'F'] {
            buf.push(ch, 230);
        }
        assert!(buf.overflow.is_some());
        let sorted: Vec<CharAndCcc> = buf.sort_and_drain().collect();
        let chars: Vec<char> = sorted.iter().map(|e| e.ch).collect();
        assert_eq!(chars, vec!['A', 'B', 'C', 'D', 'E', 'F']);
    }

    #[test]
    fn test_clear_and_reuse() {
        let mut buf = CccBuffer::new();
        buf.push('x', 10);
        buf.push('y', 20);
        buf.clear();
        assert!(buf.is_empty());
        buf.push('z', 30);
        assert_eq!(buf.len(), 1);
        assert_eq!(buf.as_slice()[0].ch, 'z');
    }

    #[test]
    fn test_single_element_sort() {
        let mut buf = CccBuffer::new();
        buf.push('\u{0301}', 230);
        let sorted: Vec<CharAndCcc> = buf.sort_and_drain().collect();
        assert_eq!(sorted.len(), 1);
        assert_eq!(sorted[0].ch, '\u{0301}');
    }

    #[test]
    fn test_already_sorted() {
        let mut buf = CccBuffer::new();
        buf.push('\u{0327}', 202);
        buf.push('\u{0323}', 220);
        buf.push('\u{0301}', 230);
        let sorted: Vec<CharAndCcc> = buf.sort_and_drain().collect();
        assert_eq!(sorted[0].ccc, 202);
        assert_eq!(sorted[1].ccc, 220);
        assert_eq!(sorted[2].ccc, 230);
    }

    #[test]
    fn test_reverse_order() {
        let mut buf = CccBuffer::new();
        buf.push('\u{0301}', 230);
        buf.push('\u{0323}', 220);
        buf.push('\u{0327}', 202);
        let sorted: Vec<CharAndCcc> = buf.sort_and_drain().collect();
        assert_eq!(sorted[0].ccc, 202);
        assert_eq!(sorted[1].ccc, 220);
        assert_eq!(sorted[2].ccc, 230);
    }

    #[test]
    fn test_insertion_sort_correctness() {
        let mut data = [
            CharAndCcc { ch: 'D', ccc: 4 },
            CharAndCcc { ch: 'B', ccc: 2 },
            CharAndCcc { ch: 'A', ccc: 1 },
            CharAndCcc { ch: 'C', ccc: 3 },
        ];
        insertion_sort_by_ccc(&mut data);
        assert_eq!(data[0].ccc, 1);
        assert_eq!(data[1].ccc, 2);
        assert_eq!(data[2].ccc, 3);
        assert_eq!(data[3].ccc, 4);
    }

    #[test]
    fn test_insertion_sort_stability() {
        let mut data = [
            CharAndCcc { ch: 'X', ccc: 230 },
            CharAndCcc { ch: 'A', ccc: 220 },
            CharAndCcc { ch: 'Y', ccc: 230 },
            CharAndCcc { ch: 'B', ccc: 220 },
        ];
        insertion_sort_by_ccc(&mut data);
        assert_eq!(data[0].ch, 'A');
        assert_eq!(data[1].ch, 'B');
        assert_eq!(data[2].ch, 'X');
        assert_eq!(data[3].ch, 'Y');
    }

    #[test]
    fn test_large_combining_sequence() {
        let mut buf = CccBuffer::new();
        let cccs: Vec<u8> = (0..100).map(|i| ((i * 37 + 13) % 255) as u8).collect();
        for (i, &ccc) in cccs.iter().enumerate() {
            let ch = char::from_u32(0xE000 + i as u32).unwrap();
            buf.push(ch, ccc);
        }
        assert_eq!(buf.len(), 100);
        assert!(buf.overflow.is_some());
        let sorted: Vec<CharAndCcc> = buf.sort_and_drain().collect();
        assert_eq!(sorted.len(), 100);
        for window in sorted.windows(2) {
            assert!(window[0].ccc <= window[1].ccc);
        }
        assert!(buf.is_empty());
    }

    #[test]
    fn test_ccc_lookup_real_data() {
        // U+0300 COMBINING GRAVE ACCENT: CCC = 230
        assert_eq!(canonical_combining_class('\u{0300}'), 230);
        // U+0327 COMBINING CEDILLA: CCC = 202
        assert_eq!(canonical_combining_class('\u{0327}'), 202);
        // ASCII 'A': CCC = 0
        assert_eq!(canonical_combining_class('A'), 0);
    }
}
