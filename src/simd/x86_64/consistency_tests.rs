//! Cross-backend consistency tests: verify that SSE4.2, AVX2, and AVX-512BW
//! produce identical bitmasks for the same input.

#[cfg(test)]
mod tests {
    use crate::simd::x86_64::{avx2, avx512, sse42};

    /// Generate a deterministic but non-trivial 64-byte test vector.
    fn make_test_data(seed: u8) -> [u8; 64] {
        let mut data = [0u8; 64];
        for i in 0..64 {
            data[i] = seed.wrapping_mul(i as u8).wrapping_add(i as u8 ^ seed);
        }
        data
    }

    #[test]
    fn sse42_vs_avx2_consistency() {
        if !std::is_x86_feature_detected!("sse4.2") || !std::is_x86_feature_detected!("avx2") {
            return;
        }
        for seed in 0..=255u8 {
            let data = make_test_data(seed);
            for bound in [0x00, 0x40, 0x80, 0xC0, 0xFF] {
                let mask_sse = unsafe { sse42::scan_chunk(data.as_ptr(), bound) };
                let mask_avx = unsafe { avx2::scan_chunk(data.as_ptr(), bound) };
                assert_eq!(
                    mask_sse, mask_avx,
                    "SSE4.2 vs AVX2 mismatch: seed={seed}, bound=0x{bound:02X}"
                );
            }
        }
    }

    #[test]
    fn sse42_vs_avx512_consistency() {
        if !std::is_x86_feature_detected!("sse4.2") || !std::is_x86_feature_detected!("avx512bw")
        {
            return;
        }
        for seed in 0..=255u8 {
            let data = make_test_data(seed);
            for bound in [0x00, 0x40, 0x80, 0xC0, 0xFF] {
                let mask_sse = unsafe { sse42::scan_chunk(data.as_ptr(), bound) };
                let mask_512 = unsafe { avx512::scan_chunk(data.as_ptr(), bound) };
                assert_eq!(
                    mask_sse, mask_512,
                    "SSE4.2 vs AVX-512BW mismatch: seed={seed}, bound=0x{bound:02X}"
                );
            }
        }
    }

    #[test]
    fn prefetch_vs_plain_sse42() {
        if !std::is_x86_feature_detected!("sse4.2") {
            return;
        }
        for seed in 0..=255u8 {
            let data = make_test_data(seed);
            let dummy = data.as_ptr();
            let plain = unsafe { sse42::scan_chunk(data.as_ptr(), 0x80) };
            let pf = unsafe { sse42::scan_and_prefetch(data.as_ptr(), dummy, dummy, 0x80) };
            assert_eq!(plain, pf, "SSE4.2 prefetch mismatch at seed={seed}");
        }
    }

    #[test]
    fn prefetch_vs_plain_avx2() {
        if !std::is_x86_feature_detected!("avx2") {
            return;
        }
        for seed in 0..=255u8 {
            let data = make_test_data(seed);
            let dummy = data.as_ptr();
            let plain = unsafe { avx2::scan_chunk(data.as_ptr(), 0x80) };
            let pf = unsafe { avx2::scan_and_prefetch(data.as_ptr(), dummy, dummy, 0x80) };
            assert_eq!(plain, pf, "AVX2 prefetch mismatch at seed={seed}");
        }
    }

    #[test]
    fn prefetch_vs_plain_avx512() {
        if !std::is_x86_feature_detected!("avx512bw") {
            return;
        }
        for seed in 0..=255u8 {
            let data = make_test_data(seed);
            let dummy = data.as_ptr();
            let plain = unsafe { avx512::scan_chunk(data.as_ptr(), 0x80) };
            let pf = unsafe { avx512::scan_and_prefetch(data.as_ptr(), dummy, dummy, 0x80) };
            assert_eq!(plain, pf, "AVX-512BW prefetch mismatch at seed={seed}");
        }
    }

    #[test]
    fn all_backends_against_scalar_reference() {
        if !std::is_x86_feature_detected!("sse4.2") {
            return;
        }

        fn scalar_scan(data: &[u8; 64], bound: u8) -> u64 {
            let mut mask = 0u64;
            for i in 0..64 {
                if data[i] >= bound {
                    mask |= 1u64 << i;
                }
            }
            mask
        }

        for seed in 0..=255u8 {
            let data = make_test_data(seed);
            for bound in [0x00, 0x01, 0x7F, 0x80, 0xBF, 0xC0, 0xFE, 0xFF] {
                let expected = scalar_scan(&data, bound);

                let sse = unsafe { sse42::scan_chunk(data.as_ptr(), bound) };
                assert_eq!(
                    sse, expected,
                    "SSE4.2 vs scalar: seed={seed}, bound=0x{bound:02X}"
                );

                if std::is_x86_feature_detected!("avx2") {
                    let avx = unsafe { avx2::scan_chunk(data.as_ptr(), bound) };
                    assert_eq!(
                        avx, expected,
                        "AVX2 vs scalar: seed={seed}, bound=0x{bound:02X}"
                    );
                }

                if std::is_x86_feature_detected!("avx512bw") {
                    let a512 = unsafe { avx512::scan_chunk(data.as_ptr(), bound) };
                    assert_eq!(
                        a512, expected,
                        "AVX-512BW vs scalar: seed={seed}, bound=0x{bound:02X}"
                    );
                }
            }
        }
    }
}
