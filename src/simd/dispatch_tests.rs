//! Integration tests for the SIMD dispatch layer.
//!
//! These call the public dispatch functions and verify correctness
//! without caring which backend was selected.

#[cfg(test)]
mod tests {
    use crate::simd;

    #[test]
    fn dispatch_scan_chunk_all_ascii() {
        let data = [0x41u8; 64];
        let mask = unsafe { simd::scan_chunk(data.as_ptr(), 0x80) };
        assert_eq!(mask, 0u64);
    }

    #[test]
    fn dispatch_scan_chunk_all_non_ascii() {
        let data = [0xC0u8; 64];
        let mask = unsafe { simd::scan_chunk(data.as_ptr(), 0x80) };
        assert_eq!(mask, u64::MAX);
    }

    #[test]
    fn dispatch_scan_chunk_mixed() {
        let mut data = [0x20u8; 64];
        data[0] = 0x80;
        data[15] = 0xFF;
        data[16] = 0xC0;
        data[31] = 0xE0;
        data[32] = 0xF0;
        data[63] = 0x80;
        let mask = unsafe { simd::scan_chunk(data.as_ptr(), 0x80) };
        let expected =
            1u64 | (1u64 << 15) | (1u64 << 16) | (1u64 << 31) | (1u64 << 32) | (1u64 << 63);
        assert_eq!(mask, expected);
    }

    #[test]
    fn dispatch_scan_chunk_boundary() {
        let mut data = [0x7Fu8; 64];
        data[0] = 0x80;
        let mask = unsafe { simd::scan_chunk(data.as_ptr(), 0x80) };
        assert_eq!(mask, 1u64);
    }

    #[test]
    fn dispatch_scan_and_prefetch() {
        let data = [0x30u8; 128];
        let mask = unsafe {
            simd::scan_and_prefetch(
                data.as_ptr(),
                data.as_ptr().add(64),
                data.as_ptr().add(64),
                0x80,
            )
        };
        assert_eq!(mask, 0u64);
    }

    #[test]
    fn dispatch_scan_and_prefetch_matches_scan_chunk() {
        let mut data = [0x30u8; 128];
        data[5] = 0xAA;
        data[22] = 0xBB;
        data[40] = 0xCC;
        data[58] = 0xDD;

        let mask_plain = unsafe { simd::scan_chunk(data.as_ptr(), 0x80) };
        let mask_pf = unsafe {
            simd::scan_and_prefetch(
                data.as_ptr(),
                data.as_ptr().add(64),
                data.as_ptr().add(64),
                0x80,
            )
        };
        assert_eq!(mask_plain, mask_pf);
    }

    #[test]
    fn dispatch_trampoline_settles() {
        let data = [0xFFu8; 64];
        for _ in 0..100 {
            let mask = unsafe { simd::scan_chunk(data.as_ptr(), 0x80) };
            assert_eq!(mask, u64::MAX);
        }
    }
}
