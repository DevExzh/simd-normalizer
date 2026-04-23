//! x86_64 SIMD backends.

#[cfg(not(any(test, feature = "internal-test-api")))]
pub(crate) mod avx2;

#[cfg(any(test, feature = "internal-test-api"))]
#[doc(hidden)]
pub mod avx2;

#[cfg(not(any(test, feature = "internal-test-api")))]
pub(crate) mod avx512;

#[cfg(any(test, feature = "internal-test-api"))]
#[doc(hidden)]
pub mod avx512;

#[cfg(all(test, feature = "std"))]
mod consistency_tests;

#[cfg(not(any(test, feature = "internal-test-api")))]
pub(crate) mod sse42;

#[cfg(any(test, feature = "internal-test-api"))]
#[doc(hidden)]
pub mod sse42;
