//! AVX2 scanner backend.
//!
//! Processes 64 bytes as 2x 256-bit vectors.

#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::{
    __m256i, _mm256_cmpeq_epi8, _mm256_loadu_si256, _mm256_max_epu8, _mm256_movemask_epi8,
    _mm256_set1_epi8,
};

/// Number of bytes per AVX2 register.
const LANES: usize = 32;

/// AVX2 vector type alias.
#[cfg(target_arch = "x86_64")]
type SimdVec = __m256i;

/// Load 32 bytes from an unaligned pointer.
///
/// # Safety
/// `ptr` must point to at least 32 readable bytes.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
#[inline]
unsafe fn simd_load(ptr: *const u8) -> SimdVec {
    unsafe { _mm256_loadu_si256(ptr as *const __m256i) }
}

/// Broadcast a single byte to all lanes.
///
/// # Safety
/// Requires AVX2.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
#[inline]
unsafe fn simd_splat(val: u8) -> SimdVec {
    _mm256_set1_epi8(val as i8)
}

/// Compare `a >= b` for unsigned bytes. Returns a bitmask with one bit per
/// lane (32 bits used).
///
/// Uses: max(a, b) == a  iff  a >= b (unsigned).
/// Cast chain: `_mm256_movemask_epi8` returns `i32`. Cast to `u32` to avoid
/// sign extension, then the macro casts to `u64`.
///
/// # Safety
/// Requires AVX2.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
#[inline]
unsafe fn simd_cmpge_mask(a: SimdVec, b: SimdVec) -> u32 {
    let max = _mm256_max_epu8(a, b);
    let eq = _mm256_cmpeq_epi8(max, a);
    _mm256_movemask_epi8(eq) as u32
}

// Invoke the scanner macro to generate `scan_chunk` and `scan_and_prefetch`.
#[cfg(target_arch = "x86_64")]
crate::simd::scanner::impl_scanner! {
    #[target_feature(enable = "avx2")]
    mod avx2
}

#[cfg(all(test, target_arch = "x86_64", feature = "std"))]
mod tests {
    use super::*;

    fn has_avx2() -> bool {
        #[cfg(target_feature = "avx2")]
        return true;
        #[cfg(not(target_feature = "avx2"))]
        return std::is_x86_feature_detected!("avx2");
    }

    #[test]
    fn avx2_scan_all_below() {
        if !has_avx2() {
            return;
        }
        let data = [0x41u8; 64];
        let mask = unsafe { scan_chunk(data.as_ptr(), 0xC0) };
        assert_eq!(mask, 0);
    }

    #[test]
    fn avx2_scan_all_above() {
        if !has_avx2() {
            return;
        }
        let data = [0xFFu8; 64];
        let mask = unsafe { scan_chunk(data.as_ptr(), 0xC0) };
        assert_eq!(mask, u64::MAX);
    }

    #[test]
    fn avx2_scan_at_bound() {
        if !has_avx2() {
            return;
        }
        let data = [0xC0u8; 64];
        let mask = unsafe { scan_chunk(data.as_ptr(), 0xC0) };
        assert_eq!(mask, u64::MAX);
    }

    #[test]
    fn avx2_scan_mixed() {
        if !has_avx2() {
            return;
        }
        let mut data = [0x41u8; 64];
        data[0] = 0xC0;
        data[63] = 0xFF;
        let mask = unsafe { scan_chunk(data.as_ptr(), 0xC0) };
        assert_eq!(mask, (1u64 << 0) | (1u64 << 63));
    }

    #[test]
    fn avx2_scan_lane_boundary() {
        if !has_avx2() {
            return;
        }
        let mut data = [0x00u8; 64];
        data[31] = 0xC0; // End of first 256-bit lane
        data[63] = 0xC0; // End of second 256-bit lane
        let mask = unsafe { scan_chunk(data.as_ptr(), 0xC0) };
        assert_eq!(mask, (1u64 << 31) | (1u64 << 63));
    }

    #[test]
    fn avx2_scan_first_byte_each_lane() {
        if !has_avx2() {
            return;
        }
        let mut data = [0x00u8; 64];
        data[0] = 0xC0;
        data[32] = 0xC0;
        let mask = unsafe { scan_chunk(data.as_ptr(), 0xC0) };
        assert_eq!(mask, (1u64 << 0) | (1u64 << 32));
    }

    #[test]
    fn avx2_scan_bound_zero() {
        if !has_avx2() {
            return;
        }
        let data = [0x00u8; 64];
        let mask = unsafe { scan_chunk(data.as_ptr(), 0x00) };
        assert_eq!(mask, u64::MAX);
    }

    #[test]
    fn avx2_scan_bound_ff() {
        if !has_avx2() {
            return;
        }
        let mut data = [0xFEu8; 64];
        data[7] = 0xFF;
        let mask = unsafe { scan_chunk(data.as_ptr(), 0xFF) };
        assert_eq!(mask, 1u64 << 7);
    }

    #[test]
    fn avx2_scan_every_position() {
        if !has_avx2() {
            return;
        }
        for pos in 0..64 {
            let mut chunk = [0u8; 64];
            chunk[pos] = 0xC0;
            let mask = unsafe { scan_chunk(chunk.as_ptr(), 0xC0) };
            assert_eq!(mask, 1u64 << pos, "AVX2: Expected only bit {pos} set");
        }
    }

    #[test]
    fn avx2_scan_and_prefetch_matches_scan_chunk() {
        if !has_avx2() {
            return;
        }
        let mut data = [0x41u8; 64];
        data[5] = 0xE0;
        data[37] = 0xD0;
        let dummy = data.as_ptr();
        let mask_plain = unsafe { scan_chunk(data.as_ptr(), 0xC0) };
        let mask_pf = unsafe { scan_and_prefetch(data.as_ptr(), dummy, dummy, 0xC0) };
        assert_eq!(
            mask_plain, mask_pf,
            "Prefetch variant must produce identical bitmask"
        );
    }

    #[test]
    fn avx2_matches_scalar() {
        if !has_avx2() {
            return;
        }
        let mut chunk = [0u8; 64];
        for (i, byte) in chunk.iter_mut().enumerate() {
            *byte = (i as u8).wrapping_mul(7);
        }
        let avx_mask = unsafe { scan_chunk(chunk.as_ptr(), 0xC0) };
        let scalar_mask = unsafe { crate::simd::scalar::scan_chunk(chunk.as_ptr(), 0xC0) };
        assert_eq!(avx_mask, scalar_mask, "AVX2 must match scalar");
    }
}
