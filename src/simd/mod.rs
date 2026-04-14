//! SIMD dispatch layer -- AtomicPtr trampoline for runtime detection,
//! compile-time cfg fallback.

pub mod scanner;
pub mod prefetch;
pub mod scalar;

#[cfg(target_arch = "x86_64")]
pub mod x86_64;

#[cfg(target_arch = "aarch64")]
pub mod aarch64;

#[cfg(target_arch = "wasm32")]
pub mod wasm32;
