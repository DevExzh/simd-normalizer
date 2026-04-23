//! aarch64 SIMD backends.

#[cfg(not(any(test, feature = "internal-test-api")))]
pub(crate) mod neon;

#[cfg(any(test, feature = "internal-test-api"))]
#[doc(hidden)]
pub mod neon;
