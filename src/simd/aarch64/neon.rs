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

// ---------------------------------------------------------------------------
// Pair scanner: process two adjacent 64-byte chunks software-pipelined.
// ---------------------------------------------------------------------------

/// Scan two adjacent 64-byte chunks software-pipelined.
///
/// Returns `(mask_a, mask_b)`, the bitmasks of `ptr_a..ptr_a+64` and
/// `ptr_b..ptr_b+64`. Each set bit indicates a byte at that position is
/// `>= bound`.
///
/// Pipelining strategy: we issue all 8 NEON loads first, then all 8
/// `vcgeq_u8` compares, then all 16 halved `vaddv_u8` reductions. Apple
/// Silicon has 32 NEON V registers and 4 NEON pipes, so the compiler can
/// freely reorder these inter-vector-independent operations across pipes.
/// This buys ~2x throughput on dense-hit chunks (CJK, Arabic, Hangul,
/// emoji) where the per-vector early-out in [`scan_chunk`] is rarely
/// taken anyway.
///
/// To preserve the ASCII fast path we still apply a coarse-grained
/// any-set check per chunk: if `vmaxvq_u8` of the OR-reduction of the
/// chunk's four compare results is zero, that chunk's mask is forced to
/// zero without running the AND/reduce chain. This costs ~3 vorr + 1
/// vmaxv (~4 cycles) per chunk and is dominated by the 16-cycle reduce
/// chain it gates.
///
/// # Safety
/// - `ptr_a` and `ptr_b` must each point to at least 64 readable bytes.
/// - NEON must be available (always true on AArch64).
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
#[inline]
pub(crate) unsafe fn scan_chunk_pair(ptr_a: *const u8, ptr_b: *const u8, bound: u8) -> (u64, u64) {
    unsafe {
        let bound_vec = simd_splat(bound);
        let bit_mask = load_bit_mask();

        // 8 loads (let the compiler schedule them across LSU pipes).
        let v0a = simd_load(ptr_a);
        let v1a = simd_load(ptr_a.add(16));
        let v2a = simd_load(ptr_a.add(32));
        let v3a = simd_load(ptr_a.add(48));
        let v0b = simd_load(ptr_b);
        let v1b = simd_load(ptr_b.add(16));
        let v2b = simd_load(ptr_b.add(32));
        let v3b = simd_load(ptr_b.add(48));

        // 8 compares.
        let c0a = vcgeq_u8(v0a, bound_vec);
        let c1a = vcgeq_u8(v1a, bound_vec);
        let c2a = vcgeq_u8(v2a, bound_vec);
        let c3a = vcgeq_u8(v3a, bound_vec);
        let c0b = vcgeq_u8(v0b, bound_vec);
        let c1b = vcgeq_u8(v1b, bound_vec);
        let c2b = vcgeq_u8(v2b, bound_vec);
        let c3b = vcgeq_u8(v3b, bound_vec);

        // Coarse any-set check per chunk: skip the AND/reduce chain on
        // empty chunks. Uses NEON's `vorrq_u8` to fold four compare
        // vectors and `vmaxvq_u8` to test for any non-zero lane in 1 op.
        let any_a = {
            use core::arch::aarch64::vorrq_u8;
            let or01 = vorrq_u8(c0a, c1a);
            let or23 = vorrq_u8(c2a, c3a);
            vmaxvq_u8(vorrq_u8(or01, or23)) != 0
        };
        let any_b = {
            use core::arch::aarch64::vorrq_u8;
            let or01 = vorrq_u8(c0b, c1b);
            let or23 = vorrq_u8(c2b, c3b);
            vmaxvq_u8(vorrq_u8(or01, or23)) != 0
        };

        let mask_a = if any_a {
            // 4 ANDs.
            let m0 = vandq_u8(c0a, bit_mask);
            let m1 = vandq_u8(c1a, bit_mask);
            let m2 = vandq_u8(c2a, bit_mask);
            let m3 = vandq_u8(c3a, bit_mask);
            // 8 halved reduces (1-cycle throughput each on Apple).
            let l0 = vaddv_u8(vget_low_u8(m0)) as u64;
            let h0 = vaddv_u8(vget_high_u8(m0)) as u64;
            let l1 = vaddv_u8(vget_low_u8(m1)) as u64;
            let h1 = vaddv_u8(vget_high_u8(m1)) as u64;
            let l2 = vaddv_u8(vget_low_u8(m2)) as u64;
            let h2 = vaddv_u8(vget_high_u8(m2)) as u64;
            let l3 = vaddv_u8(vget_low_u8(m3)) as u64;
            let h3 = vaddv_u8(vget_high_u8(m3)) as u64;
            (l0 | (h0 << 8))
                | ((l1 | (h1 << 8)) << 16)
                | ((l2 | (h2 << 8)) << 32)
                | ((l3 | (h3 << 8)) << 48)
        } else {
            0
        };

        let mask_b = if any_b {
            let m0 = vandq_u8(c0b, bit_mask);
            let m1 = vandq_u8(c1b, bit_mask);
            let m2 = vandq_u8(c2b, bit_mask);
            let m3 = vandq_u8(c3b, bit_mask);
            let l0 = vaddv_u8(vget_low_u8(m0)) as u64;
            let h0 = vaddv_u8(vget_high_u8(m0)) as u64;
            let l1 = vaddv_u8(vget_low_u8(m1)) as u64;
            let h1 = vaddv_u8(vget_high_u8(m1)) as u64;
            let l2 = vaddv_u8(vget_low_u8(m2)) as u64;
            let h2 = vaddv_u8(vget_high_u8(m2)) as u64;
            let l3 = vaddv_u8(vget_low_u8(m3)) as u64;
            let h3 = vaddv_u8(vget_high_u8(m3)) as u64;
            (l0 | (h0 << 8))
                | ((l1 | (h1 << 8)) << 16)
                | ((l2 | (h2 << 8)) << 32)
                | ((l3 | (h3 << 8)) << 48)
        } else {
            0
        };

        (mask_a, mask_b)
    }
}

/// Scan two adjacent 64-byte chunks and issue two prefetch instructions.
///
/// See [`scan_chunk_pair`] for the pipelining strategy. The `prefetch_l1`
/// and `prefetch_l2` arguments are streaming prefetches for the data ~256B
/// and ~1024B ahead respectively.
///
/// # Safety
/// Same as [`scan_chunk_pair`], plus prefetch pointers must be derived from
/// a valid allocation.
#[cfg(target_arch = "aarch64")]
#[allow(dead_code)]
#[target_feature(enable = "neon")]
#[inline(never)]
pub(crate) unsafe fn scan_chunk_pair_and_prefetch(
    ptr_a: *const u8,
    ptr_b: *const u8,
    prefetch_l1: *const u8,
    prefetch_l2: *const u8,
    bound: u8,
) -> (u64, u64) {
    use crate::simd::prefetch::{prefetch_l1_stream, prefetch_l2_stream};
    unsafe {
        prefetch_l1_stream(prefetch_l1);
        prefetch_l2_stream(prefetch_l2);
        scan_chunk_pair(ptr_a, ptr_b, bound)
    }
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

    // -- scan_chunk_pair tests ----------------------------------------------

    #[test]
    fn neon_pair_matches_two_singles() {
        // Bytes pattern that exercises sub-vector boundaries on both chunks.
        let mut data = [0u8; 128];
        for (i, b) in data.iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(13).wrapping_add(7);
        }
        unsafe {
            let (ma, mb) = scan_chunk_pair(data.as_ptr(), data.as_ptr().add(64), 0xC0);
            let m1 = scan_chunk(data.as_ptr(), 0xC0);
            let m2 = scan_chunk(data.as_ptr().add(64), 0xC0);
            assert_eq!(ma, m1);
            assert_eq!(mb, m2);
        }
    }

    #[test]
    fn neon_pair_all_below() {
        let data = [0x41u8; 128];
        unsafe {
            let (ma, mb) = scan_chunk_pair(data.as_ptr(), data.as_ptr().add(64), 0xC0);
            assert_eq!(ma, 0);
            assert_eq!(mb, 0);
        }
    }

    #[test]
    fn neon_pair_all_above() {
        let data = [0xFFu8; 128];
        unsafe {
            let (ma, mb) = scan_chunk_pair(data.as_ptr(), data.as_ptr().add(64), 0xC0);
            assert_eq!(ma, u64::MAX);
            assert_eq!(mb, u64::MAX);
        }
    }

    #[test]
    fn neon_pair_mixed_one_chunk_empty() {
        // First chunk all-ASCII (no hits), second chunk dense hits.
        let mut data = [0x41u8; 128];
        for b in &mut data[64..] {
            *b = 0xC0;
        }
        unsafe {
            let (ma, mb) = scan_chunk_pair(data.as_ptr(), data.as_ptr().add(64), 0xC0);
            assert_eq!(ma, 0);
            assert_eq!(mb, u64::MAX);
        }
    }

    #[test]
    fn neon_pair_every_position() {
        for pos in 0..128 {
            let mut data = [0u8; 128];
            data[pos] = 0xC0;
            unsafe {
                let (ma, mb) = scan_chunk_pair(data.as_ptr(), data.as_ptr().add(64), 0xC0);
                let combined = (ma as u128) | ((mb as u128) << 64);
                assert_eq!(combined, 1u128 << pos, "pair: bit {pos}");
            }
        }
    }

    #[test]
    fn neon_pair_and_prefetch_matches() {
        let mut data = [0x30u8; 128];
        data[3] = 0xD0;
        data[63] = 0xFE;
        data[64] = 0xC1;
        data[127] = 0xFF;
        let dummy = data.as_ptr();
        unsafe {
            let (ma1, mb1) = scan_chunk_pair(data.as_ptr(), data.as_ptr().add(64), 0xC0);
            let (ma2, mb2) = scan_chunk_pair_and_prefetch(
                data.as_ptr(),
                data.as_ptr().add(64),
                dummy,
                dummy,
                0xC0,
            );
            assert_eq!(ma1, ma2);
            assert_eq!(mb1, mb2);
        }
    }
}
