//! AVX-512BW scanner backend.
//!
//! Processes 64 bytes as a single 512-bit vector. AVX-512BW provides
//! native unsigned byte comparison (`_mm512_cmpge_epu8_mask`) that
//! returns `u64` directly — no movemask gymnastics needed.

#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::{__m512i, _mm512_cmpge_epu8_mask, _mm512_loadu_si512, _mm512_set1_epi8};

/// Number of bytes per AVX-512 register.
const LANES: usize = 64;

/// AVX-512 vector type alias.
#[cfg(target_arch = "x86_64")]
type SimdVec = __m512i;

/// Load 64 bytes from an unaligned pointer.
///
/// # Safety
/// `ptr` must point to at least 64 readable bytes.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512bw")]
#[inline]
unsafe fn simd_load(ptr: *const u8) -> SimdVec {
    unsafe { _mm512_loadu_si512(ptr as *const __m512i) }
}

/// Broadcast a single byte to all lanes.
///
/// # Safety
/// Requires AVX-512BW.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512bw")]
#[inline]
unsafe fn simd_splat(val: u8) -> SimdVec {
    _mm512_set1_epi8(val as i8)
}

/// Compare `a >= b` for unsigned bytes. Returns a bitmask with one bit per
/// lane (64 bits).
///
/// AVX-512BW has native unsigned comparison, so this is a single instruction.
///
/// # Safety
/// Requires AVX-512BW.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512bw")]
#[inline]
unsafe fn simd_cmpge_mask(a: SimdVec, b: SimdVec) -> u64 {
    _mm512_cmpge_epu8_mask(a, b)
}

/// Returns `true` iff any lane of `a` is `>= b`.
///
/// Thin shim built on `simd_cmpge_mask`; no algorithmic shortcut on
/// x86_64 since AVX-512 already returns a `u64` mask in one instruction.
///
/// # Safety
/// Requires AVX-512BW.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512bw")]
#[inline]
unsafe fn simd_any_ge(a: SimdVec, b: SimdVec) -> bool {
    unsafe { simd_cmpge_mask(a, b) != 0 }
}

// Invoke the scanner macro to generate `scan_chunk` and `scan_and_prefetch`.
#[cfg(target_arch = "x86_64")]
crate::simd::scanner::impl_scanner! {
    #[target_feature(enable = "avx512bw")]
    mod avx512
}

#[cfg(all(test, target_arch = "x86_64", feature = "std"))]
mod tests {
    use super::*;

    fn has_avx512bw() -> bool {
        #[cfg(target_feature = "avx512bw")]
        return true;
        #[cfg(not(target_feature = "avx512bw"))]
        return std::is_x86_feature_detected!("avx512bw");
    }

    #[test]
    fn avx512_scan_all_below() {
        if !has_avx512bw() {
            return;
        }
        let data = [0x41u8; 64];
        let mask = unsafe { scan_chunk(data.as_ptr(), 0xC0) };
        assert_eq!(mask, 0);
    }

    #[test]
    fn avx512_scan_all_above() {
        if !has_avx512bw() {
            return;
        }
        let data = [0xFFu8; 64];
        let mask = unsafe { scan_chunk(data.as_ptr(), 0xC0) };
        assert_eq!(mask, u64::MAX);
    }

    #[test]
    fn avx512_scan_at_bound() {
        if !has_avx512bw() {
            return;
        }
        let data = [0xC0u8; 64];
        let mask = unsafe { scan_chunk(data.as_ptr(), 0xC0) };
        assert_eq!(mask, u64::MAX);
    }

    #[test]
    fn avx512_scan_mixed() {
        if !has_avx512bw() {
            return;
        }
        let mut data = [0x41u8; 64];
        data[0] = 0xC0;
        data[63] = 0xFF;
        let mask = unsafe { scan_chunk(data.as_ptr(), 0xC0) };
        assert_eq!(mask, (1u64 << 0) | (1u64 << 63));
    }

    #[test]
    fn avx512_scan_single_load_covers_64() {
        if !has_avx512bw() {
            return;
        }
        let mut data = [0x00u8; 64];
        for i in (0..64).step_by(8) {
            data[i] = 0xC0;
        }
        let mask = unsafe { scan_chunk(data.as_ptr(), 0xC0) };
        let mut expected = 0u64;
        for i in (0..64).step_by(8) {
            expected |= 1u64 << i;
        }
        assert_eq!(mask, expected);
    }

    #[test]
    fn avx512_scan_bound_zero() {
        if !has_avx512bw() {
            return;
        }
        let data = [0x00u8; 64];
        let mask = unsafe { scan_chunk(data.as_ptr(), 0x00) };
        assert_eq!(mask, u64::MAX);
    }

    #[test]
    fn avx512_scan_bound_ff() {
        if !has_avx512bw() {
            return;
        }
        let mut data = [0xFEu8; 64];
        data[7] = 0xFF;
        let mask = unsafe { scan_chunk(data.as_ptr(), 0xFF) };
        assert_eq!(mask, 1u64 << 7);
    }

    #[test]
    fn avx512_scan_every_position() {
        if !has_avx512bw() {
            return;
        }
        for pos in 0..64 {
            let mut chunk = [0u8; 64];
            chunk[pos] = 0xC0;
            let mask = unsafe { scan_chunk(chunk.as_ptr(), 0xC0) };
            assert_eq!(mask, 1u64 << pos, "AVX-512: Expected only bit {pos} set");
        }
    }

    #[test]
    fn avx512_scan_and_prefetch_matches_scan_chunk() {
        if !has_avx512bw() {
            return;
        }
        let mut data = [0x41u8; 64];
        data[10] = 0xE0;
        data[50] = 0xD0;
        let dummy = data.as_ptr();
        let mask_plain = unsafe { scan_chunk(data.as_ptr(), 0xC0) };
        let mask_pf = unsafe { scan_and_prefetch(data.as_ptr(), dummy, dummy, 0xC0) };
        assert_eq!(
            mask_plain, mask_pf,
            "Prefetch variant must produce identical bitmask"
        );
    }

    #[test]
    fn avx512_matches_scalar() {
        if !has_avx512bw() {
            return;
        }
        let mut chunk = [0u8; 64];
        for (i, byte) in chunk.iter_mut().enumerate() {
            *byte = (i as u8).wrapping_mul(7);
        }
        let avx_mask = unsafe { scan_chunk(chunk.as_ptr(), 0xC0) };
        let scalar_mask = unsafe { crate::simd::scalar::scan_chunk(chunk.as_ptr(), 0xC0) };
        assert_eq!(avx_mask, scalar_mask, "AVX-512BW must match scalar");
    }
}
