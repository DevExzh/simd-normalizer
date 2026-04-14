//! simd-normalizer -- SIMD-accelerated Unicode normalization.
//!
//! Provides NFC, NFD, NFKC, NFKD normalization with a single-pass
//! SIMD-guided architecture.  The core is `no_std + alloc`; enable
//! the `std` feature for runtime CPU dispatch.

#![no_std]
#![warn(missing_docs)]
#![allow(unused)] // temporary: stubs only

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

pub mod ccc;
pub mod compose;
pub mod decompose;
pub mod hangul;
pub mod normalizer;
pub mod quick_check;
pub mod simd;
pub mod tables;
pub mod utf8;
