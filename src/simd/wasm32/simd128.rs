//! wasm simd128 scanner backend.
//!
//! Processes 64 bytes as 4x 128-bit vectors. wasm simd128 provides
//! native `u8x16_ge` for unsigned comparison and `i8x16_bitmask` for
//! extracting a 16-bit mask — no movemask emulation needed.

#[cfg(target_arch = "wasm32")]
use core::arch::wasm32::{i8x16_bitmask, u8x16_ge, u8x16_splat, v128, v128_load};

/// Number of bytes per wasm simd128 register.
const LANES: usize = 16;

/// wasm simd128 vector type alias.
#[cfg(target_arch = "wasm32")]
type SimdVec = v128;

/// Load 16 bytes from a pointer.
///
/// # Safety
/// `ptr` must point to at least 16 readable bytes.
#[cfg(target_arch = "wasm32")]
#[target_feature(enable = "simd128")]
#[inline]
unsafe fn simd_load(ptr: *const u8) -> SimdVec {
    unsafe { v128_load(ptr as *const v128) }
}

/// Broadcast a single byte to all lanes.
///
/// # Safety
/// Requires simd128.
#[cfg(target_arch = "wasm32")]
#[target_feature(enable = "simd128")]
#[inline]
unsafe fn simd_splat(val: u8) -> SimdVec {
    u8x16_splat(val)
}

/// Compare `a >= b` for unsigned bytes. Returns a bitmask with one bit per
/// lane (16 bits used).
///
/// `i8x16_bitmask` extracts the high bit of each byte from the comparison
/// result (0xFF where true, 0x00 where false), producing a u16 bitmask.
///
/// # Safety
/// Requires simd128.
#[cfg(target_arch = "wasm32")]
#[target_feature(enable = "simd128")]
#[inline]
unsafe fn simd_cmpge_mask(a: SimdVec, b: SimdVec) -> u32 {
    let cmp = u8x16_ge(a, b);
    i8x16_bitmask(cmp) as u16 as u32
}

/// Returns `true` iff any lane of `a` is `>= b`.
///
/// Thin shim built on `simd_cmpge_mask`; wasm `i8x16_bitmask` is already
/// a single op, so no dedicated reduction wins here.
///
/// # Safety
/// Requires simd128.
#[cfg(target_arch = "wasm32")]
#[target_feature(enable = "simd128")]
#[inline]
unsafe fn simd_any_ge(a: SimdVec, b: SimdVec) -> bool {
    unsafe { simd_cmpge_mask(a, b) != 0 }
}

// Invoke the scanner macro to generate `scan_chunk` and `scan_and_prefetch`.
#[cfg(target_arch = "wasm32")]
crate::simd::scanner::impl_scanner! {
    #[target_feature(enable = "simd128")]
    mod simd128
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;

    #[test]
    fn simd128_scan_all_below() {
        let data = [0x41u8; 64];
        let mask = unsafe { scan_chunk(data.as_ptr(), 0xC0) };
        assert_eq!(mask, 0);
    }

    #[test]
    fn simd128_scan_all_above() {
        let data = [0xFFu8; 64];
        let mask = unsafe { scan_chunk(data.as_ptr(), 0xC0) };
        assert_eq!(mask, u64::MAX);
    }

    #[test]
    fn simd128_scan_mixed() {
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
    fn simd128_scan_every_position() {
        for pos in 0..64 {
            let mut chunk = [0u8; 64];
            chunk[pos] = 0xC0;
            let mask = unsafe { scan_chunk(chunk.as_ptr(), 0xC0) };
            assert_eq!(mask, 1u64 << pos, "simd128: Expected only bit {pos} set");
        }
    }

    #[test]
    fn simd128_scan_and_prefetch_matches() {
        let mut data = [0x30u8; 64];
        data[7] = 0xD0;
        data[31] = 0xE5;
        let dummy = data.as_ptr();
        let m1 = unsafe { scan_chunk(data.as_ptr(), 0xC0) };
        let m2 = unsafe { scan_and_prefetch(data.as_ptr(), dummy, dummy, 0xC0) };
        assert_eq!(m1, m2, "Prefetch variant must produce identical bitmask");
    }

    #[test]
    fn simd128_matches_scalar() {
        let mut chunk = [0u8; 64];
        for i in 0..64 {
            chunk[i] = (i as u8).wrapping_mul(7);
        }
        let simd_mask = unsafe { scan_chunk(chunk.as_ptr(), 0xC0) };
        let scalar_mask = unsafe { crate::simd::scalar::scan_chunk(chunk.as_ptr(), 0xC0) };
        assert_eq!(simd_mask, scalar_mask, "wasm simd128 must match scalar");
    }
}
