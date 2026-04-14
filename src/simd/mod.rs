//! SIMD-accelerated byte scanning and dispatch.
//!
//! This module provides the scanner infrastructure that produces per-chunk
//! bitmasks for the normalizer's main loop. The scanner identifies byte
//! positions >= a passthrough bound, flagging them for scalar normalization.
//!
//! Backends:
//! - `scalar` -- always available, no special CPU features
//! - `x86_64::sse42`, `x86_64::avx2`, `x86_64::avx512`
//! - `aarch64::neon`
//! - `wasm32::simd128`
//!
//! The dispatch layer (`scan_chunk` / `scan_and_prefetch`) selects the best
//! backend:
//! - x86_64 + std: runtime CPUID via self-replacing AtomicPtr trampoline
//! - x86_64 + no_std: compile-time via `cfg(target_feature)`
//! - aarch64: always NEON (mandatory in AArch64 ISA)
//! - wasm32: simd128 if compiled with the feature, else scalar
//! - other: scalar fallback

pub(crate) mod prefetch;
pub(crate) mod scalar;
pub(crate) mod scanner;

#[cfg(target_arch = "x86_64")]
pub(crate) mod x86_64;

#[cfg(target_arch = "aarch64")]
pub(crate) mod aarch64;

#[cfg(target_arch = "wasm32")]
pub(crate) mod wasm32;

// ---------------------------------------------------------------------------
// Function pointer types for dispatch
// ---------------------------------------------------------------------------

/// Function pointer type for `scan_chunk(ptr, bound) -> mask`.
#[allow(dead_code)]
type ScanChunkFn = unsafe fn(*const u8, u8) -> u64;

/// Function pointer type for `scan_and_prefetch(ptr, l1, l2, bound) -> mask`.
#[allow(dead_code)]
type ScanAndPrefetchFn = unsafe fn(*const u8, *const u8, *const u8, u8) -> u64;

// ===========================================================================
// x86_64 + std: runtime dispatch via AtomicPtr trampoline
// ===========================================================================
#[cfg(all(feature = "std", target_arch = "x86_64"))]
mod dispatch {
    use super::*;
    use std::sync::atomic::{AtomicPtr, Ordering};

    type FnRaw = *mut ();

    static SCAN_IMPL: AtomicPtr<()> = AtomicPtr::new(detect_scan as FnRaw);
    #[allow(dead_code)]
    static PREFETCH_IMPL: AtomicPtr<()> = AtomicPtr::new(detect_prefetch as FnRaw);

    unsafe fn detect_scan(ptr: *const u8, bound: u8) -> u64 {
        let f = pick_best_scan();
        SCAN_IMPL.store(f as FnRaw, Ordering::Relaxed);
        unsafe { f(ptr, bound) }
    }

    fn pick_best_scan() -> ScanChunkFn {
        if std::is_x86_feature_detected!("avx512bw") {
            return super::x86_64::avx512::scan_chunk;
        }
        if std::is_x86_feature_detected!("avx2") {
            return super::x86_64::avx2::scan_chunk;
        }
        if std::is_x86_feature_detected!("sse4.2") {
            return super::x86_64::sse42::scan_chunk;
        }
        super::scalar::scan_chunk
    }

    #[allow(dead_code)]
    unsafe fn detect_prefetch(
        ptr: *const u8,
        prefetch_l1: *const u8,
        prefetch_l2: *const u8,
        bound: u8,
    ) -> u64 {
        let f = pick_best_prefetch();
        PREFETCH_IMPL.store(f as FnRaw, Ordering::Relaxed);
        unsafe { f(ptr, prefetch_l1, prefetch_l2, bound) }
    }

    #[allow(dead_code)]
    fn pick_best_prefetch() -> ScanAndPrefetchFn {
        if std::is_x86_feature_detected!("avx512bw") {
            return super::x86_64::avx512::scan_and_prefetch;
        }
        if std::is_x86_feature_detected!("avx2") {
            return super::x86_64::avx2::scan_and_prefetch;
        }
        if std::is_x86_feature_detected!("sse4.2") {
            return super::x86_64::sse42::scan_and_prefetch;
        }
        super::scalar::scan_and_prefetch
    }

    #[inline]
    pub(crate) unsafe fn scan_chunk(ptr: *const u8, bound: u8) -> u64 {
        let f: ScanChunkFn = unsafe { core::mem::transmute(SCAN_IMPL.load(Ordering::Relaxed)) };
        unsafe { f(ptr, bound) }
    }

    #[allow(dead_code)]
    #[inline]
    pub(crate) unsafe fn scan_and_prefetch(
        ptr: *const u8,
        prefetch_l1: *const u8,
        prefetch_l2: *const u8,
        bound: u8,
    ) -> u64 {
        let f: ScanAndPrefetchFn =
            unsafe { core::mem::transmute(PREFETCH_IMPL.load(Ordering::Relaxed)) };
        unsafe { f(ptr, prefetch_l1, prefetch_l2, bound) }
    }
}

// ===========================================================================
// x86_64 + no_std: compile-time dispatch via cfg(target_feature)
// ===========================================================================
#[cfg(all(not(feature = "std"), target_arch = "x86_64"))]
mod dispatch {
    #[inline]
    pub(crate) unsafe fn scan_chunk(ptr: *const u8, bound: u8) -> u64 {
        #[cfg(target_feature = "avx512bw")]
        {
            return unsafe { super::x86_64::avx512::scan_chunk(ptr, bound) };
        }
        #[cfg(all(target_feature = "avx2", not(target_feature = "avx512bw")))]
        {
            return unsafe { super::x86_64::avx2::scan_chunk(ptr, bound) };
        }
        #[cfg(all(
            target_feature = "sse4.2",
            not(target_feature = "avx2"),
            not(target_feature = "avx512bw")
        ))]
        {
            return unsafe { super::x86_64::sse42::scan_chunk(ptr, bound) };
        }
        #[cfg(not(any(
            target_feature = "sse4.2",
            target_feature = "avx2",
            target_feature = "avx512bw"
        )))]
        {
            unsafe { super::scalar::scan_chunk(ptr, bound) }
        }
    }

    #[allow(dead_code)]
    #[inline]
    pub(crate) unsafe fn scan_and_prefetch(
        ptr: *const u8,
        prefetch_l1: *const u8,
        prefetch_l2: *const u8,
        bound: u8,
    ) -> u64 {
        #[cfg(target_feature = "avx512bw")]
        {
            return unsafe {
                super::x86_64::avx512::scan_and_prefetch(ptr, prefetch_l1, prefetch_l2, bound)
            };
        }
        #[cfg(all(target_feature = "avx2", not(target_feature = "avx512bw")))]
        {
            return unsafe {
                super::x86_64::avx2::scan_and_prefetch(ptr, prefetch_l1, prefetch_l2, bound)
            };
        }
        #[cfg(all(
            target_feature = "sse4.2",
            not(target_feature = "avx2"),
            not(target_feature = "avx512bw")
        ))]
        {
            return unsafe {
                super::x86_64::sse42::scan_and_prefetch(ptr, prefetch_l1, prefetch_l2, bound)
            };
        }
        #[cfg(not(any(
            target_feature = "sse4.2",
            target_feature = "avx2",
            target_feature = "avx512bw"
        )))]
        {
            unsafe { super::scalar::scan_and_prefetch(ptr, prefetch_l1, prefetch_l2, bound) }
        }
    }
}

// ===========================================================================
// aarch64: NEON is always available
// ===========================================================================
#[cfg(target_arch = "aarch64")]
mod dispatch {
    #[inline]
    pub(crate) unsafe fn scan_chunk(ptr: *const u8, bound: u8) -> u64 {
        unsafe { super::aarch64::neon::scan_chunk(ptr, bound) }
    }

    #[allow(dead_code)]
    #[inline]
    pub(crate) unsafe fn scan_and_prefetch(
        ptr: *const u8,
        prefetch_l1: *const u8,
        prefetch_l2: *const u8,
        bound: u8,
    ) -> u64 {
        unsafe { super::aarch64::neon::scan_and_prefetch(ptr, prefetch_l1, prefetch_l2, bound) }
    }
}

// ===========================================================================
// wasm32: simd128 is a compile-time decision
// ===========================================================================
#[cfg(target_arch = "wasm32")]
mod dispatch {
    #[inline]
    pub(crate) unsafe fn scan_chunk(ptr: *const u8, bound: u8) -> u64 {
        #[cfg(target_feature = "simd128")]
        {
            return unsafe { super::wasm32::simd128::scan_chunk(ptr, bound) };
        }
        #[cfg(not(target_feature = "simd128"))]
        {
            return unsafe { super::scalar::scan_chunk(ptr, bound) };
        }
    }

    #[allow(dead_code)]
    #[inline]
    pub(crate) unsafe fn scan_and_prefetch(
        ptr: *const u8,
        prefetch_l1: *const u8,
        prefetch_l2: *const u8,
        bound: u8,
    ) -> u64 {
        #[cfg(target_feature = "simd128")]
        {
            return unsafe {
                super::wasm32::simd128::scan_and_prefetch(ptr, prefetch_l1, prefetch_l2, bound)
            };
        }
        #[cfg(not(target_feature = "simd128"))]
        {
            return unsafe {
                super::scalar::scan_and_prefetch(ptr, prefetch_l1, prefetch_l2, bound)
            };
        }
    }
}

// ===========================================================================
// Fallback: any other architecture -> scalar
// ===========================================================================
#[cfg(not(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    target_arch = "wasm32",
)))]
mod dispatch {
    #[inline]
    pub(crate) unsafe fn scan_chunk(ptr: *const u8, bound: u8) -> u64 {
        unsafe { super::scalar::scan_chunk(ptr, bound) }
    }

    #[allow(dead_code)]
    #[inline]
    pub(crate) unsafe fn scan_and_prefetch(
        ptr: *const u8,
        prefetch_l1: *const u8,
        prefetch_l2: *const u8,
        bound: u8,
    ) -> u64 {
        unsafe { super::scalar::scan_and_prefetch(ptr, prefetch_l1, prefetch_l2, bound) }
    }
}

// ===========================================================================
// Public dispatch API
// ===========================================================================

/// Scan a 64-byte chunk, returning a bitmask of bytes >= `bound`.
///
/// Dispatches to the best available SIMD backend for the current platform.
/// On x86_64 with `std`, the first call triggers runtime CPUID detection;
/// subsequent calls are near-zero overhead (one relaxed atomic load + indirect call).
///
/// # Safety
/// - `ptr` must be valid for 64 bytes of read access.
#[inline]
pub(crate) unsafe fn scan_chunk(ptr: *const u8, bound: u8) -> u64 {
    unsafe { dispatch::scan_chunk(ptr, bound) }
}

/// Scan a 64-byte chunk and issue prefetch hints for upcoming data.
///
/// Dispatches to the best available SIMD backend for the current platform.
///
/// # Safety
/// - `ptr` must be valid for 64 bytes of read access.
/// - Prefetch pointers may be out-of-bounds (prefetch is a non-faulting hint
///   on all supported architectures).
#[allow(dead_code)]
#[inline]
pub(crate) unsafe fn scan_and_prefetch(
    ptr: *const u8,
    prefetch_l1: *const u8,
    prefetch_l2: *const u8,
    bound: u8,
) -> u64 {
    unsafe { dispatch::scan_and_prefetch(ptr, prefetch_l1, prefetch_l2, bound) }
}

/// Find the byte offset of the first byte >= `bound` in `bytes`.
///
/// Uses the dispatched SIMD scanner in 64-byte chunks, then scans the tail
/// byte-by-byte. Returns `bytes.len()` if no such byte exists.
#[allow(dead_code)]
#[inline]
pub(crate) fn find_first_above(bytes: &[u8], bound: u8) -> usize {
    let len = bytes.len();
    let mut offset = 0usize;

    // Process full 64-byte chunks via the dispatch layer.
    while offset + 64 <= len {
        // SAFETY: offset + 64 <= len, so the pointer is valid for 64 bytes.
        let mask = unsafe { scan_chunk(bytes.as_ptr().add(offset), bound) };
        if mask != 0 {
            return offset + mask.trailing_zeros() as usize;
        }
        offset += 64;
    }

    // Scan remaining bytes one at a time.
    while offset < len {
        if bytes[offset] >= bound {
            return offset;
        }
        offset += 1;
    }

    len
}

#[cfg(test)]
mod dispatch_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_first_above_all_below() {
        let data = b"Hello, world! This is pure ASCII and should return len.";
        assert_eq!(find_first_above(data, 0xC0), data.len());
    }

    #[test]
    fn find_first_above_first_byte() {
        let data = [0xC0u8, 0x00, 0x00, 0x00];
        assert_eq!(find_first_above(&data, 0xC0), 0);
    }

    #[test]
    fn find_first_above_in_second_chunk() {
        let mut data = [0u8; 128];
        data[65] = 0xC0;
        assert_eq!(find_first_above(&data, 0xC0), 65);
    }

    #[test]
    fn find_first_above_in_tail() {
        let mut data = [0u8; 70];
        data[68] = 0xC0;
        assert_eq!(find_first_above(&data, 0xC0), 68);
    }

    #[test]
    fn find_first_above_empty() {
        assert_eq!(find_first_above(&[], 0xC0), 0);
    }

    #[test]
    fn find_first_above_exact_chunk_boundary() {
        let mut data = [0u8; 64];
        data[63] = 0xC0;
        assert_eq!(find_first_above(&data, 0xC0), 63);
    }

    #[test]
    fn find_first_above_multi_chunk_utf8() {
        let s = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\u{00E9}";
        let bytes = s.as_bytes();
        assert_eq!(find_first_above(bytes, 0xC0), 64);
    }
}
