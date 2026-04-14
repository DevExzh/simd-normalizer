#![allow(dead_code)]
//! Platform-specific software prefetch wrappers.
//!
//! These issue prefetch hints to warm cache lines before the SIMD scanner
//! reaches them. On platforms without prefetch support (WASM, generic) the
//! functions are no-ops.

/// Distance in 64-byte chunks to prefetch into L2.
pub(crate) const PREFETCH_L2_DISTANCE: usize = 16; // ~1024 bytes

/// Distance in 64-byte chunks to prefetch into L1.
pub(crate) const PREFETCH_L1_DISTANCE: usize = 4; // ~256 bytes

/// Scanner chunk size in bytes.
pub(crate) const CHUNK_SIZE: usize = 64;

// ---------------------------------------------------------------------------
// x86 / x86_64
// ---------------------------------------------------------------------------

/// Prefetch for L2 cache (streaming read, temporal locality hint T1).
///
/// # Safety
/// `ptr` must point into a readable allocation (may be past the end by up to
/// one cache line -- prefetch of an unmapped page is architecturally a no-op
/// on x86 but the pointer itself must be derived from a valid allocation).
#[cfg(target_arch = "x86_64")]
#[inline(always)]
pub(crate) unsafe fn prefetch_l2_stream(ptr: *const u8) {
    use core::arch::x86_64::{_MM_HINT_T1, _mm_prefetch};
    unsafe { _mm_prefetch(ptr as *const i8, _MM_HINT_T1) };
}

/// Prefetch for L1 cache (streaming read, temporal locality hint T0).
///
/// # Safety
/// Same as [`prefetch_l2_stream`].
#[cfg(target_arch = "x86_64")]
#[inline(always)]
pub(crate) unsafe fn prefetch_l1_stream(ptr: *const u8) {
    use core::arch::x86_64::{_MM_HINT_T0, _mm_prefetch};
    unsafe { _mm_prefetch(ptr as *const i8, _MM_HINT_T0) };
}

/// Prefetch for write-allocate (non-temporal hint -- output buffer).
///
/// # Safety
/// Same as [`prefetch_l2_stream`].
#[cfg(target_arch = "x86_64")]
#[inline(always)]
pub(crate) unsafe fn prefetch_write(ptr: *const u8) {
    use core::arch::x86_64::{_MM_HINT_NTA, _mm_prefetch};
    unsafe { _mm_prefetch(ptr as *const i8, _MM_HINT_NTA) };
}

// ---------------------------------------------------------------------------
// AArch64
// ---------------------------------------------------------------------------

/// Prefetch for L2 cache (streaming read).
///
/// # Safety
/// `ptr` must be derived from a valid allocation.
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub(crate) unsafe fn prefetch_l2_stream(ptr: *const u8) {
    unsafe {
        core::arch::asm!(
            "prfm pldl2strm, [{ptr}]",
            ptr = in(reg) ptr,
            options(nostack, preserves_flags),
        );
    }
}

/// Prefetch for L1 cache (streaming read).
///
/// # Safety
/// Same as [`prefetch_l2_stream`].
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub(crate) unsafe fn prefetch_l1_stream(ptr: *const u8) {
    unsafe {
        core::arch::asm!(
            "prfm pldl1strm, [{ptr}]",
            ptr = in(reg) ptr,
            options(nostack, preserves_flags),
        );
    }
}

/// Prefetch for write-allocate (L1 keep).
///
/// # Safety
/// Same as [`prefetch_l2_stream`].
#[cfg(target_arch = "aarch64")]
#[inline(always)]
pub(crate) unsafe fn prefetch_write(ptr: *const u8) {
    unsafe {
        core::arch::asm!(
            "prfm pstl1keep, [{ptr}]",
            ptr = in(reg) ptr,
            options(nostack, preserves_flags),
        );
    }
}

// ---------------------------------------------------------------------------
// WASM32 and all other architectures -- no-ops
// ---------------------------------------------------------------------------

/// Prefetch for L2 cache -- no-op on this platform.
///
/// # Safety
/// Always safe (no-op).
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
#[inline(always)]
pub(crate) unsafe fn prefetch_l2_stream(_ptr: *const u8) {}

/// Prefetch for L1 cache -- no-op on this platform.
///
/// # Safety
/// Always safe (no-op).
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
#[inline(always)]
pub(crate) unsafe fn prefetch_l1_stream(_ptr: *const u8) {}

/// Prefetch for write-allocate -- no-op on this platform.
///
/// # Safety
/// Always safe (no-op).
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
#[inline(always)]
pub(crate) unsafe fn prefetch_write(_ptr: *const u8) {}
