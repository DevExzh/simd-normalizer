//! simd-normalizer -- SIMD-accelerated Unicode normalization.
//!
//! Provides NFC, NFD, NFKC, NFKD normalization with a single-pass
//! SIMD-guided architecture.  The core is `no_std + alloc`; enable
//! the `std` feature for runtime CPU dispatch.

#![no_std]
#![warn(missing_docs)]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

use alloc::borrow::Cow;

pub mod casefold;
mod ccc;
mod compose;
pub mod confusable;
mod decompose;
mod hangul;
pub mod matching;
pub mod normalizer;
mod quick_check;
#[cfg(not(any(test, feature = "internal-test-api")))]
pub(crate) mod simd;

#[cfg(any(test, feature = "internal-test-api"))]
#[doc(hidden)]
pub mod simd;
mod tables;
mod utf8;

#[cfg(any(test, feature = "internal-test-api"))]
pub mod tables_ext;

/// Crate-private SIMD wrappers re-exported for integration tests.
/// Not for downstream use; semver-exempt; tracks `simd::scan_chunk*`
/// signatures exactly.
#[cfg(any(test, feature = "internal-test-api"))]
#[allow(rustdoc::private_intra_doc_links)]
pub mod simd_test_api {
    /// See [`crate::simd::scan_chunk`].
    /// # Safety
    /// `ptr` must be valid for 64 bytes of read access.
    #[inline]
    pub unsafe fn scan_chunk(ptr: *const u8, bound: u8) -> u64 {
        unsafe { crate::simd::scan_chunk(ptr, bound) }
    }

    /// Direct NEON `scan_chunk` for cross-vtable consistency tests on
    /// aarch64. Mirrors the dispatched signature.
    /// # Safety
    /// `ptr` must be valid for 64 bytes of read access.
    #[cfg(target_arch = "aarch64")]
    #[inline]
    pub unsafe fn neon_scan_chunk(ptr: *const u8, bound: u8) -> u64 {
        unsafe { crate::simd::aarch64::neon::scan_chunk(ptr, bound) }
    }

    /// Direct SVE2 `scan_chunk` for cross-vtable consistency tests on
    /// aarch64. Mirrors the dispatched signature. The caller MUST verify
    /// `is_aarch64_feature_detected!("sve2")` before invoking this on a
    /// std target — calling it on a host without SVE2 is undefined
    /// behaviour (SIGILL).
    /// # Safety
    /// `ptr` must be valid for 64 bytes of read access AND the host must
    /// support SVE2.
    #[cfg(target_arch = "aarch64")]
    #[inline]
    pub unsafe fn sve2_scan_chunk(ptr: *const u8, bound: u8) -> u64 {
        unsafe { crate::simd::aarch64::sve2::scan_chunk(ptr, bound) }
    }
}

pub use casefold::{CaseFoldMode, casefold, casefold_char};
pub use confusable::{are_confusable, skeleton};
#[cfg(any(test, feature = "internal-test-api"))]
pub use matching::normalize_for_matching_legacy;
pub use matching::{
    MatchingOptions, matches_normalized, normalize_for_matching, normalize_for_matching_utf16,
};
pub use normalizer::{NfcNormalizer, NfdNormalizer, NfkcNormalizer, NfkdNormalizer};
pub use quick_check::IsNormalized;

#[cfg(feature = "quick_check_oracle")]
pub use crate::quick_check::{
    quick_check_nfc, quick_check_nfc_oracle, quick_check_nfd, quick_check_nfd_oracle,
    quick_check_nfkc, quick_check_nfkc_oracle, quick_check_nfkd, quick_check_nfkd_oracle,
};

/// Return a pre-built NFC normalizer.
#[inline]
pub fn nfc() -> NfcNormalizer {
    NfcNormalizer::new()
}

/// Return a pre-built NFD normalizer.
#[inline]
pub fn nfd() -> NfdNormalizer {
    NfdNormalizer::new()
}

/// Return a pre-built NFKC normalizer.
#[inline]
pub fn nfkc() -> NfkcNormalizer {
    NfkcNormalizer::new()
}

/// Return a pre-built NFKD normalizer.
#[inline]
pub fn nfkd() -> NfkdNormalizer {
    NfkdNormalizer::new()
}

/// Convenience trait for normalizing `&str` slices.
///
/// All methods return `Cow<'_, str>`, which is `Cow::Borrowed` when the input
/// is already in the target normalization form (zero allocation).
pub trait UnicodeNormalization {
    /// Normalize to NFC (Canonical Decomposition, followed by Canonical Composition).
    fn nfc(&self) -> Cow<'_, str>;
    /// Normalize to NFD (Canonical Decomposition).
    fn nfd(&self) -> Cow<'_, str>;
    /// Normalize to NFKC (Compatibility Decomposition, followed by Canonical Composition).
    fn nfkc(&self) -> Cow<'_, str>;
    /// Normalize to NFKD (Compatibility Decomposition).
    fn nfkd(&self) -> Cow<'_, str>;
    /// Check whether the string is already in NFC.
    fn is_nfc(&self) -> bool;
    /// Check whether the string is already in NFD.
    fn is_nfd(&self) -> bool;
    /// Check whether the string is already in NFKC.
    fn is_nfkc(&self) -> bool;
    /// Check whether the string is already in NFKD.
    fn is_nfkd(&self) -> bool;
}

impl UnicodeNormalization for str {
    #[inline]
    fn nfc(&self) -> Cow<'_, str> {
        crate::nfc().normalize(self)
    }
    #[inline]
    fn nfd(&self) -> Cow<'_, str> {
        crate::nfd().normalize(self)
    }
    #[inline]
    fn nfkc(&self) -> Cow<'_, str> {
        crate::nfkc().normalize(self)
    }
    #[inline]
    fn nfkd(&self) -> Cow<'_, str> {
        crate::nfkd().normalize(self)
    }
    #[inline]
    fn is_nfc(&self) -> bool {
        crate::nfc().is_normalized(self)
    }
    #[inline]
    fn is_nfd(&self) -> bool {
        crate::nfd().is_normalized(self)
    }
    #[inline]
    fn is_nfkc(&self) -> bool {
        crate::nfkc().is_normalized(self)
    }
    #[inline]
    fn is_nfkd(&self) -> bool {
        crate::nfkd().is_normalized(self)
    }
}
