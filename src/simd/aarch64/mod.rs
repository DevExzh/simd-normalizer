//! aarch64 SIMD backends.

/// Bit-position mask shared between the NEON and SVE2 movemask reductions.
///
/// Each byte holds `1 << (position & 7)`, repeated across the two halves so
/// the same 16-byte vector can be applied to either half via NEON's halved
/// `vaddv_u8` reduction (NEON path) or via SVE2's NEON-aliased low 128 bits
/// after the predicate-to-byte expansion (SVE2 path).
#[cfg(target_arch = "aarch64")]
pub(crate) const MOVEMASK_BIT_MASK: [u8; 16] =
    [1, 2, 4, 8, 16, 32, 64, 128, 1, 2, 4, 8, 16, 32, 64, 128];

#[cfg(not(any(test, feature = "internal-test-api")))]
pub(crate) mod neon;

#[cfg(any(test, feature = "internal-test-api"))]
#[doc(hidden)]
pub mod neon;

#[cfg(not(any(test, feature = "internal-test-api")))]
pub(crate) mod sve2;

#[cfg(any(test, feature = "internal-test-api"))]
#[doc(hidden)]
pub mod sve2;
