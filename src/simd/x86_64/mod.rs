//! x86_64 SIMD backends.

pub(crate) mod sse42;
pub(crate) mod avx2;
pub(crate) mod avx512;
#[cfg(test)]
mod consistency_tests;
