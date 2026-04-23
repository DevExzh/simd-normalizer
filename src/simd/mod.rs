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
//! The dispatch layer selects the best backend via a `SimdVTable` struct:
//! - x86_64 + std: runtime CPUID via `OnceLock<&'static SimdVTable>`
//! - x86_64 + no_std: compile-time via `cfg(target_feature)`
//! - aarch64: always NEON (mandatory in AArch64 ISA)
//! - wasm32: simd128 if compiled with the feature, else scalar
//! - other: scalar fallback

pub(crate) mod prefetch;
pub(crate) mod scanner;

#[cfg(not(any(test, feature = "internal-test-api")))]
pub(crate) mod scalar;

#[cfg(any(test, feature = "internal-test-api"))]
#[doc(hidden)]
pub mod scalar;

#[cfg(all(
    target_arch = "x86_64",
    not(any(test, feature = "internal-test-api"))
))]
pub(crate) mod x86_64;

#[cfg(all(target_arch = "x86_64", any(test, feature = "internal-test-api")))]
#[doc(hidden)]
pub mod x86_64;

#[cfg(all(
    target_arch = "aarch64",
    not(any(test, feature = "internal-test-api"))
))]
pub(crate) mod aarch64;

#[cfg(all(target_arch = "aarch64", any(test, feature = "internal-test-api")))]
#[doc(hidden)]
pub mod aarch64;

#[cfg(all(
    target_arch = "wasm32",
    not(any(test, feature = "internal-test-api"))
))]
pub(crate) mod wasm32;

#[cfg(all(target_arch = "wasm32", any(test, feature = "internal-test-api")))]
#[doc(hidden)]
pub mod wasm32;

// ---------------------------------------------------------------------------
// SimdVTable -- extensible dispatch table for all SIMD operations
// ---------------------------------------------------------------------------

/// VTable holding all SIMD-dispatched function pointers.
///
/// A single `&'static SimdVTable` reference is resolved once (at runtime on
/// x86_64+std, at compile time elsewhere) and reused for every subsequent call.
/// Future fused pipeline operations (e.g. scan + case-fold) will be added here.
pub(crate) struct SimdVTable {
    pub scan_chunk: unsafe fn(*const u8, u8) -> u64,
    pub scan_and_prefetch: unsafe fn(*const u8, *const u8, *const u8, u8) -> u64,
}

// ---------------------------------------------------------------------------
// Static VTable instances -- one per architecture level
// ---------------------------------------------------------------------------

#[allow(dead_code)]
static VTABLE_SCALAR: SimdVTable = SimdVTable {
    scan_chunk: scalar::scan_chunk,
    scan_and_prefetch: scalar::scan_and_prefetch,
};

#[cfg(target_arch = "x86_64")]
#[allow(dead_code)]
static VTABLE_SSE42: SimdVTable = SimdVTable {
    scan_chunk: x86_64::sse42::scan_chunk,
    scan_and_prefetch: x86_64::sse42::scan_and_prefetch,
};

#[cfg(target_arch = "x86_64")]
#[allow(dead_code)]
static VTABLE_AVX2: SimdVTable = SimdVTable {
    scan_chunk: x86_64::avx2::scan_chunk,
    scan_and_prefetch: x86_64::avx2::scan_and_prefetch,
};

#[cfg(target_arch = "x86_64")]
#[allow(dead_code)]
static VTABLE_AVX512: SimdVTable = SimdVTable {
    scan_chunk: x86_64::avx512::scan_chunk,
    scan_and_prefetch: x86_64::avx512::scan_and_prefetch,
};

#[cfg(target_arch = "aarch64")]
static VTABLE_NEON: SimdVTable = SimdVTable {
    scan_chunk: aarch64::neon::scan_chunk,
    scan_and_prefetch: aarch64::neon::scan_and_prefetch,
};

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
static VTABLE_SIMD128: SimdVTable = SimdVTable {
    scan_chunk: wasm32::simd128::scan_chunk,
    scan_and_prefetch: wasm32::simd128::scan_and_prefetch,
};

// ===========================================================================
// x86_64 + std: runtime dispatch via OnceLock
// ===========================================================================
#[cfg(all(feature = "std", target_arch = "x86_64"))]
mod dispatch {
    use super::SimdVTable;
    use std::sync::OnceLock;

    static VTABLE: OnceLock<&'static SimdVTable> = OnceLock::new();

    fn detect_best() -> &'static SimdVTable {
        if std::is_x86_feature_detected!("avx512bw") {
            return &super::VTABLE_AVX512;
        }
        if std::is_x86_feature_detected!("avx2") {
            return &super::VTABLE_AVX2;
        }
        if std::is_x86_feature_detected!("sse4.2") {
            return &super::VTABLE_SSE42;
        }
        &super::VTABLE_SCALAR
    }

    #[inline]
    pub(crate) fn get_vtable() -> &'static SimdVTable {
        VTABLE.get_or_init(detect_best)
    }
}

// ===========================================================================
// x86_64 + no_std: compile-time dispatch via cfg(target_feature)
// ===========================================================================
#[cfg(all(not(feature = "std"), target_arch = "x86_64"))]
mod dispatch {
    use super::SimdVTable;

    #[inline]
    pub(crate) fn get_vtable() -> &'static SimdVTable {
        #[cfg(target_feature = "avx512bw")]
        {
            return &super::VTABLE_AVX512;
        }
        #[cfg(all(target_feature = "avx2", not(target_feature = "avx512bw")))]
        {
            return &super::VTABLE_AVX2;
        }
        #[cfg(all(
            target_feature = "sse4.2",
            not(target_feature = "avx2"),
            not(target_feature = "avx512bw")
        ))]
        {
            return &super::VTABLE_SSE42;
        }
        #[cfg(not(any(
            target_feature = "sse4.2",
            target_feature = "avx2",
            target_feature = "avx512bw"
        )))]
        {
            &super::VTABLE_SCALAR
        }
    }
}

// ===========================================================================
// aarch64: NEON is always available
// ===========================================================================
#[cfg(target_arch = "aarch64")]
mod dispatch {
    use super::SimdVTable;

    #[inline]
    pub(crate) fn get_vtable() -> &'static SimdVTable {
        &super::VTABLE_NEON
    }
}

// ===========================================================================
// wasm32: simd128 is a compile-time decision
// ===========================================================================
#[cfg(target_arch = "wasm32")]
mod dispatch {
    use super::SimdVTable;

    #[inline]
    pub(crate) fn get_vtable() -> &'static SimdVTable {
        #[cfg(target_feature = "simd128")]
        {
            return &super::VTABLE_SIMD128;
        }
        #[cfg(not(target_feature = "simd128"))]
        {
            return &super::VTABLE_SCALAR;
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
    use super::SimdVTable;

    #[inline]
    pub(crate) fn get_vtable() -> &'static SimdVTable {
        &super::VTABLE_SCALAR
    }
}

// ===========================================================================
// Public dispatch API
// ===========================================================================

/// Scan a 64-byte chunk, returning a bitmask of bytes >= `bound`.
///
/// Dispatches to the best available SIMD backend for the current platform.
/// On x86_64 with `std`, the first call triggers runtime CPUID detection via
/// `OnceLock`; subsequent calls are a single pointer dereference.
///
/// # Safety
/// - `ptr` must be valid for 64 bytes of read access.
#[inline]
pub(crate) unsafe fn scan_chunk(ptr: *const u8, bound: u8) -> u64 {
    let vt = dispatch::get_vtable();
    unsafe { (vt.scan_chunk)(ptr, bound) }
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
    let vt = dispatch::get_vtable();
    unsafe { (vt.scan_and_prefetch)(ptr, prefetch_l1, prefetch_l2, bound) }
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

    #[test]
    fn vtable_get_returns_consistent_reference() {
        let vt1 = dispatch::get_vtable();
        let vt2 = dispatch::get_vtable();
        assert!(
            core::ptr::eq(vt1, vt2),
            "get_vtable() must return the same reference"
        );
    }
}
