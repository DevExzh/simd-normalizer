//! SIMD scanner macro.
//!
//! Generates `scan_chunk` and `scan_and_prefetch` functions for each backend.
//! Each backend must provide, in the invoking module:
//!
//! - A type `SimdVec` (the SIMD vector register type).
//! - `unsafe fn simd_load(ptr: *const u8) -> SimdVec`
//! - `unsafe fn simd_splat(val: u8) -> SimdVec`
//! - `unsafe fn simd_cmpge_mask(a: SimdVec, b: SimdVec) -> u32` (or u64)
//!   Returns a bitmask with one bit per lane.
//! - `unsafe fn simd_any_ge(a: SimdVec, b: SimdVec) -> bool`
//!   Returns whether any lane of `a` is `>= b`. Used for the empty-chunk
//!   early-out on the ASCII/Latin-1 hot path.
//! - `const LANES: usize` (bytes per vector).
//!
//! For backends with LANES < 64, the macro loads multiple vectors per chunk
//! and assembles the sub-masks into a single `u64`.

/// Generate scanner functions for a SIMD backend.
///
/// Usage:
/// ```ignore
/// impl_scanner! {
///     #[target_feature(enable = "sse4.2")]
///     mod sse42
/// }
/// ```
macro_rules! impl_scanner {
    ($(#[$feat:meta])* mod $name:ident) => {
        /// Scan a 64-byte chunk. Returns a `u64` bitmask where set bits indicate
        /// byte positions with values >= `bound`.
        ///
        /// # Safety
        /// - `ptr` must point to at least 64 readable bytes.
        /// - The required CPU feature must be available.
        $(#[$feat])*
        #[inline]
        pub(crate) unsafe fn scan_chunk(ptr: *const u8, bound: u8) -> u64 {
            let bound_vec = unsafe { simd_splat(bound) };

            const VECS_PER_CHUNK: usize = 64 / LANES;
            let mut mask: u64 = 0;

            let mut i = 0;
            while i < VECS_PER_CHUNK {
                let v = unsafe { simd_load(ptr.add(i * LANES)) };
                // Empty-chunk early-out: skip the AND/reduce chain when the
                // sub-vector has no hits. This is the ASCII/Latin-1 hot
                // path; on NEON `simd_any_ge` is a single `vmaxvq_u8` and
                // saves the entire movemask reduction.
                if unsafe { simd_any_ge(v, bound_vec) } {
                    let sub_mask = unsafe { simd_cmpge_mask(v, bound_vec) } as u64;
                    mask |= sub_mask << (i * LANES);
                }
                i += 1;
            }

            mask
        }

        /// Scan a 64-byte chunk and issue prefetch instructions.
        ///
        /// # Safety
        /// Same as [`scan_chunk`], plus `prefetch_l1` and `prefetch_l2` must
        /// point into (or one cache line past) a readable allocation.
        #[allow(dead_code)]
        $(#[$feat])*
        #[inline(never)]
        pub(crate) unsafe fn scan_and_prefetch(
            ptr: *const u8,
            prefetch_l1: *const u8,
            prefetch_l2: *const u8,
            bound: u8,
        ) -> u64 {
            use crate::simd::prefetch::{prefetch_l1_stream, prefetch_l2_stream};

            unsafe {
                prefetch_l1_stream(prefetch_l1);
                prefetch_l2_stream(prefetch_l2);
            }

            unsafe { scan_chunk(ptr, bound) }
        }
    };
}

pub(crate) use impl_scanner;
