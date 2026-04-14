//! Scalar fallback scanner -- processes 64 bytes one at a time.
//!
//! This is the baseline implementation used on platforms without SIMD support
//! and as the reference for testing SIMD backends.

/// Scan a 64-byte region starting at `ptr`. Returns a `u64` bitmask where
/// bit `i` is set if `*ptr.add(i) >= bound`.
///
/// The implementation processes 8 bytes per iteration for better ILP.
///
/// # Safety
/// - `ptr` must point to at least 64 readable bytes.
/// - No alignment requirement.
#[inline]
pub(crate) unsafe fn scan_chunk(ptr: *const u8, bound: u8) -> u64 {
    let mut mask: u64 = 0;
    let mut group = 0usize;
    while group < 8 {
        let base = group * 8;
        let mut byte_idx = 0usize;
        while byte_idx < 8 {
            let b = unsafe { *ptr.add(base + byte_idx) };
            if b >= bound {
                mask |= 1u64 << (base + byte_idx);
            }
            byte_idx += 1;
        }
        group += 1;
    }
    mask
}

/// Scan with prefetch. In the scalar backend the prefetch pointers are
/// ignored.
///
/// # Safety
/// Same as [`scan_chunk`].
#[allow(dead_code)]
#[inline]
pub(crate) unsafe fn scan_and_prefetch(
    ptr: *const u8,
    _prefetch_l1: *const u8,
    _prefetch_l2: *const u8,
    bound: u8,
) -> u64 {
    unsafe { scan_chunk(ptr, bound) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_chunk(fill: u8) -> [u8; 64] {
        [fill; 64]
    }

    #[test]
    fn scan_all_ascii_below_bound() {
        let chunk = make_chunk(0x41);
        let mask = unsafe { scan_chunk(chunk.as_ptr(), 0xC0) };
        assert_eq!(mask, 0, "All-ASCII chunk should produce mask=0");
    }

    #[test]
    fn scan_all_above_bound() {
        let chunk = make_chunk(0xFF);
        let mask = unsafe { scan_chunk(chunk.as_ptr(), 0xC0) };
        assert_eq!(
            mask,
            u64::MAX,
            "All-high chunk should produce mask=all-ones"
        );
    }

    #[test]
    fn scan_single_high_byte_at_position_0() {
        let mut chunk = make_chunk(0x00);
        chunk[0] = 0xC0;
        let mask = unsafe { scan_chunk(chunk.as_ptr(), 0xC0) };
        assert_eq!(mask, 1u64, "Only bit 0 should be set");
    }

    #[test]
    fn scan_single_high_byte_at_position_63() {
        let mut chunk = make_chunk(0x00);
        chunk[63] = 0xC0;
        let mask = unsafe { scan_chunk(chunk.as_ptr(), 0xC0) };
        assert_eq!(mask, 1u64 << 63, "Only bit 63 should be set");
    }

    #[test]
    fn scan_mixed_pattern() {
        let mut chunk = make_chunk(0x20);
        chunk[3] = 0xCC;
        chunk[17] = 0xE0;
        chunk[42] = 0xF0;
        chunk[63] = 0xFF;
        let mask = unsafe { scan_chunk(chunk.as_ptr(), 0xC0) };
        let expected = (1u64 << 3) | (1u64 << 17) | (1u64 << 42) | (1u64 << 63);
        assert_eq!(mask, expected);
    }

    #[test]
    fn scan_boundary_value_equal_to_bound() {
        let mut chunk = make_chunk(0x00);
        chunk[10] = 0xC0;
        let mask = unsafe { scan_chunk(chunk.as_ptr(), 0xC0) };
        assert_eq!(mask, 1u64 << 10);
    }

    #[test]
    fn scan_boundary_value_one_below_bound() {
        let mut chunk = make_chunk(0x00);
        chunk[10] = 0xBF;
        let mask = unsafe { scan_chunk(chunk.as_ptr(), 0xC0) };
        assert_eq!(mask, 0);
    }

    #[test]
    fn scan_and_prefetch_matches_scan_chunk() {
        let mut chunk = make_chunk(0x30);
        chunk[7] = 0xD0;
        chunk[31] = 0xE5;
        let m1 = unsafe { scan_chunk(chunk.as_ptr(), 0xC0) };
        let m2 = unsafe { scan_and_prefetch(chunk.as_ptr(), chunk.as_ptr(), chunk.as_ptr(), 0xC0) };
        assert_eq!(m1, m2, "scan_and_prefetch must match scan_chunk for scalar");
    }

    #[test]
    fn scan_alternating_pattern() {
        let mut chunk = [0u8; 64];
        for (i, byte) in chunk.iter_mut().enumerate() {
            *byte = if i % 2 == 0 { 0xFF } else { 0x00 };
        }
        let mask = unsafe { scan_chunk(chunk.as_ptr(), 0xC0) };
        let expected: u64 = 0x5555_5555_5555_5555;
        assert_eq!(mask, expected);
    }

    #[test]
    fn scan_every_position() {
        for pos in 0..64 {
            let mut chunk = make_chunk(0x00);
            chunk[pos] = 0xC0;
            let mask = unsafe { scan_chunk(chunk.as_ptr(), 0xC0) };
            assert_eq!(mask, 1u64 << pos, "Expected only bit {pos} set");
        }
    }

    #[test]
    fn scan_all_positions_set() {
        let chunk = make_chunk(0x80);
        let mask = unsafe { scan_chunk(chunk.as_ptr(), 0x80) };
        assert_eq!(mask, u64::MAX);
        let mask = unsafe { scan_chunk(chunk.as_ptr(), 0x81) };
        assert_eq!(mask, 0);
    }
}
