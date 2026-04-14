//! NEON scanner backend.
//!
//! Processes 64 bytes as 4x 128-bit vectors. NEON provides native
//! `vcgeq_u8` for unsigned comparison but lacks a direct movemask
//! instruction; we emulate it using AND + horizontal pairwise addition.

#[cfg(target_arch = "aarch64")]
use core::arch::aarch64::{
    uint8x16_t, vandq_u8, vcgeq_u8, vdupq_n_u8, vgetq_lane_u8, vld1q_u8, vpaddq_u8,
};

/// Number of bytes per NEON register.
const LANES: usize = 16;

/// NEON vector type alias.
#[cfg(target_arch = "aarch64")]
type SimdVec = uint8x16_t;

/// Bit-position mask for movemask emulation.
/// Each byte contains 2^(position % 8): [1, 2, 4, 8, 16, 32, 64, 128, ...].
#[cfg(target_arch = "aarch64")]
const BIT_MASK: [u8; 16] = [1, 2, 4, 8, 16, 32, 64, 128, 1, 2, 4, 8, 16, 32, 64, 128];

/// Load 16 bytes from an unaligned pointer.
///
/// # Safety
/// `ptr` must point to at least 16 readable bytes.
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
#[inline]
unsafe fn simd_load(ptr: *const u8) -> SimdVec {
    unsafe { vld1q_u8(ptr) }
}

/// Broadcast a single byte to all lanes.
///
/// # Safety
/// Requires NEON (always available on AArch64).
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
#[inline]
unsafe fn simd_splat(val: u8) -> SimdVec {
    unsafe { vdupq_n_u8(val) }
}

/// Compare `a >= b` for unsigned bytes. Returns a bitmask with one bit per
/// lane (16 bits used).
///
/// NEON movemask emulation:
/// 1. `vcgeq_u8` sets each byte to 0xFF or 0x00.
/// 2. AND with bit-position mask [1,2,4,8,16,32,64,128,1,2,4,8,...].
/// 3. Three rounds of `vpaddq_u8` pairwise add to compress 16 bytes into 2.
/// 4. Extract the two result bytes as a u16.
///
/// # Safety
/// Requires NEON (always available on AArch64).
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
#[inline]
unsafe fn simd_cmpge_mask(a: SimdVec, b: SimdVec) -> u32 {
    unsafe {
        let cmp = vcgeq_u8(a, b);
        let bit_mask = vld1q_u8(BIT_MASK.as_ptr());
        let masked = vandq_u8(cmp, bit_mask);
        // Three rounds of horizontal pairwise addition:
        // 16 bytes -> 8 -> 4 -> 2
        let p1 = vpaddq_u8(masked, masked);
        let p2 = vpaddq_u8(p1, p1);
        let p3 = vpaddq_u8(p2, p2);
        // byte 0 = bits 0-7, byte 1 = bits 8-15
        let lo = vgetq_lane_u8(p3, 0) as u32;
        let hi = vgetq_lane_u8(p3, 1) as u32;
        lo | (hi << 8)
    }
}

// Invoke the scanner macro to generate `scan_chunk` and `scan_and_prefetch`.
#[cfg(target_arch = "aarch64")]
crate::simd::scanner::impl_scanner! {
    #[target_feature(enable = "neon")]
    mod neon
}

#[cfg(all(test, target_arch = "aarch64"))]
mod tests {
    use super::*;

    #[test]
    fn neon_scan_all_below() {
        let data = [0x41u8; 64];
        let mask = unsafe { scan_chunk(data.as_ptr(), 0xC0) };
        assert_eq!(mask, 0);
    }

    #[test]
    fn neon_scan_all_above() {
        let data = [0xFFu8; 64];
        let mask = unsafe { scan_chunk(data.as_ptr(), 0xC0) };
        assert_eq!(mask, u64::MAX);
    }

    #[test]
    fn neon_scan_at_bound() {
        let data = [0xC0u8; 64];
        let mask = unsafe { scan_chunk(data.as_ptr(), 0xC0) };
        assert_eq!(mask, u64::MAX);
    }

    #[test]
    fn neon_scan_mixed() {
        let mut data = [0x41u8; 64];
        data[0] = 0xC0;
        data[15] = 0xC0;
        data[16] = 0xC0;
        data[63] = 0xFF;
        let mask = unsafe { scan_chunk(data.as_ptr(), 0xC0) };
        let expected = (1u64 << 0) | (1u64 << 15) | (1u64 << 16) | (1u64 << 63);
        assert_eq!(mask, expected);
    }

    #[test]
    fn neon_scan_every_position() {
        for pos in 0..64 {
            let mut chunk = [0u8; 64];
            chunk[pos] = 0xC0;
            let mask = unsafe { scan_chunk(chunk.as_ptr(), 0xC0) };
            assert_eq!(mask, 1u64 << pos, "NEON: Expected only bit {pos} set");
        }
    }

    #[test]
    fn neon_scan_bound_zero() {
        let data = [0x00u8; 64];
        let mask = unsafe { scan_chunk(data.as_ptr(), 0x00) };
        assert_eq!(mask, u64::MAX);
    }

    #[test]
    fn neon_scan_and_prefetch_matches() {
        let mut data = [0x30u8; 64];
        data[7] = 0xD0;
        data[31] = 0xE5;
        let dummy = data.as_ptr();
        let m1 = unsafe { scan_chunk(data.as_ptr(), 0xC0) };
        let m2 = unsafe { scan_and_prefetch(data.as_ptr(), dummy, dummy, 0xC0) };
        assert_eq!(m1, m2, "Prefetch variant must produce identical bitmask");
    }

    #[test]
    fn neon_matches_scalar() {
        let mut chunk = [0u8; 64];
        for i in 0..64 {
            chunk[i] = (i as u8).wrapping_mul(7);
        }
        let neon_mask = unsafe { scan_chunk(chunk.as_ptr(), 0xC0) };
        let scalar_mask = unsafe { crate::simd::scalar::scan_chunk(chunk.as_ptr(), 0xC0) };
        assert_eq!(neon_mask, scalar_mask, "NEON must match scalar");
    }
}
