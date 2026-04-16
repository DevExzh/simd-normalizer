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
pub(crate) mod simd;
mod tables;
mod utf8;

pub use casefold::{CaseFoldMode, casefold, casefold_char};
pub use confusable::{are_confusable, skeleton};
pub use matching::{
    MatchingOptions, matches_normalized, normalize_for_matching, normalize_for_matching_utf16,
};
pub use normalizer::{NfcNormalizer, NfdNormalizer, NfkcNormalizer, NfkdNormalizer};
pub use quick_check::IsNormalized;

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
