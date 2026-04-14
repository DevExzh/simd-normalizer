//! SSE 4.2 scanner backend.
//!
//! Processes 64 bytes as 4x 128-bit vectors.

#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::{
    __m128i, _mm_cmpeq_epi8, _mm_loadu_si128, _mm_max_epu8, _mm_movemask_epi8, _mm_set1_epi8,
};

/// Number of bytes per SSE register.
const LANES: usize = 16;

/// SSE vector type alias.
#[cfg(target_arch = "x86_64")]
type SimdVec = __m128i;

/// Load 16 bytes from an unaligned pointer.
///
/// # Safety
/// `ptr` must point to at least 16 readable bytes.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.2")]
#[inline]
unsafe fn simd_load(ptr: *const u8) -> SimdVec {
    unsafe { _mm_loadu_si128(ptr as *const __m128i) }
}

/// Broadcast a single byte to all lanes.
///
/// # Safety
/// Requires SSE4.2.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.2")]
#[inline]
unsafe fn simd_splat(val: u8) -> SimdVec {
    unsafe { _mm_set1_epi8(val as i8) }
}

/// Compare `a >= b` for unsigned bytes. Returns a bitmask with one bit per
/// lane (16 bits used).
///
/// Uses: max(a, b) == a  iff  a >= b (unsigned).
///
/// # Safety
/// Requires SSE4.2.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.2")]
#[inline]
unsafe fn simd_cmpge_mask(a: SimdVec, b: SimdVec) -> u32 {
    unsafe {
        let max = _mm_max_epu8(a, b);
        let eq = _mm_cmpeq_epi8(max, a);
        _mm_movemask_epi8(eq) as u16 as u32
    }
}

// Invoke the scanner macro to generate `scan_chunk` and `scan_and_prefetch`.
#[cfg(target_arch = "x86_64")]
crate::simd::scanner::impl_scanner! {
    #[target_feature(enable = "sse4.2")]
    mod sse42
}

#[cfg(all(test, target_arch = "x86_64"))]
mod tests {
    use super::*;

    fn has_sse42() -> bool {
        #[cfg(target_feature = "sse4.2")]
        return true;
        #[cfg(not(target_feature = "sse4.2"))]
        return std::is_x86_feature_detected!("sse4.2");
    }

    #[test]
    fn sse42_scan_all_ascii() {
        if !has_sse42() {
            return;
        }
        let chunk = [0x41u8; 64];
        let mask = unsafe { scan_chunk(chunk.as_ptr(), 0xC0) };
        assert_eq!(mask, 0);
    }

    #[test]
    fn sse42_scan_all_above() {
        if !has_sse42() {
            return;
        }
        let chunk = [0xFFu8; 64];
        let mask = unsafe { scan_chunk(chunk.as_ptr(), 0xC0) };
        assert_eq!(mask, u64::MAX);
    }

    #[test]
    fn sse42_scan_mixed() {
        if !has_sse42() {
            return;
        }
        let mut chunk = [0x20u8; 64];
        chunk[0] = 0xC0;
        chunk[15] = 0xC0;
        chunk[16] = 0xC0;
        chunk[63] = 0xFF;
        let mask = unsafe { scan_chunk(chunk.as_ptr(), 0xC0) };
        let expected = (1u64 << 0) | (1u64 << 15) | (1u64 << 16) | (1u64 << 63);
        assert_eq!(mask, expected);
    }

    #[test]
    fn sse42_scan_every_position() {
        if !has_sse42() {
            return;
        }
        for pos in 0..64 {
            let mut chunk = [0u8; 64];
            chunk[pos] = 0xC0;
            let mask = unsafe { scan_chunk(chunk.as_ptr(), 0xC0) };
            assert_eq!(mask, 1u64 << pos, "SSE4.2: Expected only bit {pos} set");
        }
    }

    #[test]
    fn sse42_scan_and_prefetch_matches_scan_chunk() {
        if !has_sse42() {
            return;
        }
        let mut chunk = [0x30u8; 64];
        chunk[7] = 0xD0;
        chunk[31] = 0xE5;
        let dummy = chunk.as_ptr();
        let m1 = unsafe { scan_chunk(chunk.as_ptr(), 0xC0) };
        let m2 = unsafe { scan_and_prefetch(chunk.as_ptr(), dummy, dummy, 0xC0) };
        assert_eq!(m1, m2, "Prefetch variant must produce identical bitmask");
    }

    #[test]
    fn sse42_matches_scalar() {
        if !has_sse42() {
            return;
        }
        let mut chunk = [0u8; 64];
        for i in 0..64 {
            chunk[i] = (i as u8).wrapping_mul(7);
        }
        let sse_mask = unsafe { scan_chunk(chunk.as_ptr(), 0xC0) };
        let scalar_mask = unsafe { crate::simd::scalar::scan_chunk(chunk.as_ptr(), 0xC0) };
        assert_eq!(sse_mask, scalar_mask, "SSE4.2 must match scalar");
    }
}
