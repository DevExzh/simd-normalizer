//! x86_64 SIMD backends.

pub(crate) mod avx2;
pub(crate) mod avx512;
#[cfg(all(test, feature = "std"))]
mod consistency_tests;
pub(crate) mod sse42;
