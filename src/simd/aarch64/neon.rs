//! NEON scanner backend.
//!
//! Processes 64 bytes as 4x 128-bit vectors. NEON provides native
//! `vcgeq_u8` for unsigned comparison but lacks a direct movemask
//! instruction; we emulate it using AND + halved horizontal reduction
//! via `vaddv_u8`.

#[cfg(target_arch = "aarch64")]
use core::arch::aarch64::{
    uint8x16_t, vaddv_u8, vandq_u8, vcgeq_u8, vdupq_n_u8, vget_high_u8, vget_low_u8, vld1q_u8,
    vmaxvq_u8,
};

/// Number of bytes per NEON register.
const LANES: usize = 16;

/// NEON vector type alias.
#[cfg(target_arch = "aarch64")]
type SimdVec = uint8x16_t;

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
    vdupq_n_u8(val)
}

/// Load the bit-position mask once. Marked `#[inline]` so LLVM hoists the
/// load out of the macro-generated per-vector loop.
///
/// # Safety
/// Requires NEON (always available on AArch64).
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
#[inline]
unsafe fn load_bit_mask() -> SimdVec {
    unsafe { vld1q_u8(super::MOVEMASK_BIT_MASK.as_ptr()) }
}

/// Compare `a >= b` for unsigned bytes. Returns a bitmask with one bit per
/// lane (16 bits used).
///
/// NEON movemask emulation (vaddv halves):
/// 1. `vcgeq_u8` sets each byte to 0xFF or 0x00.
/// 2. AND with bit-position mask [1,2,4,8,16,32,64,128,1,2,4,8,...].
/// 3. `vget_low_u8` / `vget_high_u8` split into two 8-byte halves (free
///    aliasing on Apple/Cortex).
/// 4. `vaddv_u8` reduces each half to a single byte (1 instruction each).
/// 5. OR-shift the two byte results into the final `u32`.
///
/// Total: ~4 ops vs. the previous ~9-op `vandq + vpaddq×3 + vget×2` chain.
///
/// # Safety
/// Requires NEON (always available on AArch64).
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
#[inline]
unsafe fn simd_cmpge_mask(a: SimdVec, b: SimdVec) -> u32 {
    unsafe {
        let cmp = vcgeq_u8(a, b);
        let bit_mask = load_bit_mask();
        let masked = vandq_u8(cmp, bit_mask);
        let lo = vaddv_u8(vget_low_u8(masked)) as u32;
        let hi = vaddv_u8(vget_high_u8(masked)) as u32;
        lo | (hi << 8)
    }
}

/// Returns `true` iff any lane of `a` is `>= b`.
///
/// Used for the empty-chunk early-out: a single `vmaxvq_u8` collapses 16
/// compare-result lanes into one byte, letting the scanner skip the AND +
/// `vaddv` chain on ASCII/Latin-1 hot paths.
///
/// # Safety
/// Requires NEON (always available on AArch64).
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
#[inline]
unsafe fn simd_any_ge(a: SimdVec, b: SimdVec) -> bool {
    vmaxvq_u8(vcgeq_u8(a, b)) != 0
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
        for (i, byte) in chunk.iter_mut().enumerate() {
            *byte = (i as u8).wrapping_mul(7);
        }
        let neon_mask = unsafe { scan_chunk(chunk.as_ptr(), 0xC0) };
        let scalar_mask = unsafe { crate::simd::scalar::scan_chunk(chunk.as_ptr(), 0xC0) };
        assert_eq!(neon_mask, scalar_mask, "NEON must match scalar");
    }

    #[test]
    fn neon_any_ge_helper() {
        unsafe {
            let zeros = simd_splat(0x00);
            let ones = simd_splat(0xFF);
            let bound = simd_splat(0xC0);
            assert!(!simd_any_ge(zeros, bound));
            assert!(simd_any_ge(ones, bound));
            assert!(simd_any_ge(bound, bound));
        }
    }
}
