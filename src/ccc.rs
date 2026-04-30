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
///
/// Bumped from 18 to 32 to absorb the worst-case fixture (`'a'` + 30 marks
/// repeated) without spilling to a per-group `Vec` allocation. The struct is
/// 8 bytes (`char` + `u8` + padding); 32 entries cost 256 bytes on the stack,
/// which lives in `normalize_impl`'s frame and is well within budget.
const INLINE_CAP: usize = 32;

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
    #[cfg(test)]
    pub(crate) fn sort_and_drain(&mut self) -> SortedDrain<'_> {
        if let Some(ref mut vec) = self.overflow {
            vec.sort_by_key(|e| e.ccc);
        } else {
            insertion_sort_by_ccc(&mut self.inline[..self.len]);
        }
        SortedDrain { buf: self, pos: 0 }
    }

    /// Sort entries by CCC (stable sort) in place. Buffer remains populated
    /// and can be iterated via `as_slice()`.
    #[inline]
    pub(crate) fn sort_in_place(&mut self) {
        if let Some(ref mut vec) = self.overflow {
            vec.sort_by_key(|e| e.ccc);
        } else {
            insertion_sort_by_ccc(&mut self.inline[..self.len]);
        }
    }

    /// Clear the buffer for reuse. Overflow Vec capacity is preserved.
    #[inline]
    pub(crate) fn clear(&mut self) {
        if let Some(ref mut vec) = self.overflow {
            vec.clear();
        }
        self.len = 0;
    }

    /// If the buffer has exactly one entry in inline storage, return it and
    /// reset the buffer to empty. Returns `None` if empty, has multiple entries,
    /// or has overflowed to heap. This is the fast path for the common case of
    /// a single combining mark following a starter (e.g., precomposed Latin → base + accent).
    #[inline(always)]
    pub(crate) fn take_single_inline(&mut self) -> Option<CharAndCcc> {
        if self.len == 1 && self.overflow.is_none() {
            self.len = 0;
            Some(self.inline[0])
        } else {
            None
        }
    }

    /// Access elements as a slice.
    #[inline]
    pub(crate) fn as_slice(&self) -> &[CharAndCcc] {
        if let Some(ref vec) = self.overflow {
            &vec[..]
        } else {
            &self.inline[..self.len]
        }
    }
}

/// Draining iterator over a CccBuffer after sorting.
#[cfg(test)]
pub(crate) struct SortedDrain<'a> {
    buf: &'a mut CccBuffer,
    pos: usize,
}

#[cfg(test)]
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

#[cfg(test)]
impl ExactSizeIterator for SortedDrain<'_> {}

/// Stable insertion sort by CCC. Optimal for small arrays (n <= ~32).
///
/// For larger inputs the call-site dispatches into a counting-sort-by-CCC,
/// but in the inline-buffer path (which is bounded by `INLINE_CAP`) insertion
/// sort wins on the constant factor: `CharAndCcc` is 8 bytes, so the entire
/// inline buffer fits in 4 cache lines and every shift is a single 64-bit
/// move. Stable.
#[inline]
fn insertion_sort_by_ccc(slice: &mut [CharAndCcc]) {
    let n = slice.len();
    let mut i = 1;
    while i < n {
        // Hoist the key out of `slice` so the inner loop's reads don't alias
        // its writes (this lets the optimizer use a single register).
        let key = slice[i];
        let key_ccc = key.ccc;
        let mut j = i;
        // SAFETY: The loop body only indexes `j-1` when `j > 0`, and `j <= i < n`,
        // so all indices stay in-bounds. We use unchecked accesses to drop the
        // panic-on-OOB bounds checks that block tight inner-loop scheduling on
        // aarch64.
        unsafe {
            while j > 0 && slice.get_unchecked(j - 1).ccc > key_ccc {
                let prev = *slice.get_unchecked(j - 1);
                *slice.get_unchecked_mut(j) = prev;
                j -= 1;
            }
            *slice.get_unchecked_mut(j) = key;
        }
        i += 1;
    }
}

/// Look up the Canonical Combining Class for a character.
#[allow(dead_code)]
#[inline]
pub(crate) fn canonical_combining_class(c: char) -> u8 {
    tables::lookup_ccc(c)
}

#[cfg(test)]
mod tests {
    use super::*;
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
        // With INLINE_CAP=32, we need >32 entries to trigger overflow.
        let n = INLINE_CAP + 1;
        for i in 0..n {
            let ch = char::from_u32(0xE000 + i as u32).unwrap();
            buf.push(ch, (200 + (i % 50)) as u8);
        }
        assert_eq!(buf.len(), n);
        assert!(buf.overflow.is_some());
        let sorted: Vec<CharAndCcc> = buf.sort_and_drain().collect();
        assert_eq!(sorted.len(), n);
        for window in sorted.windows(2) {
            assert!(window[0].ccc <= window[1].ccc);
        }
        assert!(buf.is_empty());
    }

    #[test]
    fn test_overflow_stability() {
        let mut buf = CccBuffer::new();
        // Push >INLINE_CAP entries to trigger overflow, all same CCC for stability check.
        let n = (INLINE_CAP as u32) + 2;
        let chars: Vec<char> = (0..n)
            .map(|i| char::from_u32(0xE000 + i).unwrap())
            .collect();
        for &ch in &chars {
            buf.push(ch, 230);
        }
        assert!(buf.overflow.is_some());
        let sorted: Vec<CharAndCcc> = buf.sort_and_drain().collect();
        let sorted_chars: Vec<char> = sorted.iter().map(|e| e.ch).collect();
        assert_eq!(sorted_chars, chars);
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
