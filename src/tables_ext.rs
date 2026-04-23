//! Test-only re-exports for tables crate-private helpers. Feature-gated so
//! it is not part of the public API.
#![cfg(any(test, feature = "internal-test-api"))]

use crate::decompose::DecompForm;

/// Raw trie value for a character's canonical decomposition entry.
pub fn raw_decomp_trie_value_canonical(ch: char) -> u32 {
    crate::tables::raw_decomp_trie_value(ch, DecompForm::Canonical)
}

/// Raw trie value for a supplementary-plane code point's canonical decomposition
/// entry. `cp` must be in the range `0x10000..=0x10FFFF`.
pub fn raw_decomp_trie_value_supp_canonical(cp: u32) -> u32 {
    debug_assert!((0x10000..=0x10FFFF).contains(&cp));
    // Safety: caller-documented precondition that `cp` is supplementary.
    unsafe { crate::tables::raw_decomp_trie_value_supplementary(cp, DecompForm::Canonical) }
}

/// The new packed bit; set iff the codepoint needs a starter-shadow in compose mode.
pub fn needs_starter_shadow_bit(tv: u32) -> bool {
    crate::tables::needs_starter_shadow(tv)
}

/// The legacy rule computed at runtime (slow -- tests only): CCC > 0. When the
/// next codepoint after a passthrough run is any combining mark, the compose
/// mode passthrough tail must feed its final starter into NormState so later
/// reordering / composition can still see it.
pub fn legacy_needs_starter_shadow(ch: char) -> bool {
    crate::tables::lookup_ccc(ch) != 0
}

/// Packed CCC+QC bit-shift constants (crate-private; mirrored here for
/// integration tests that audit the safe-lead byte set). Values are kept
/// in lockstep with the `pub(crate)` originals in `src/tables/mod.rs`.
pub const CCC_QC_NFC_SHIFT: u32 = crate::tables::CCC_QC_NFC_SHIFT;
/// See [`CCC_QC_NFC_SHIFT`].
pub const CCC_QC_NFD_SHIFT: u32 = crate::tables::CCC_QC_NFD_SHIFT;
/// See [`CCC_QC_NFC_SHIFT`].
pub const CCC_QC_NFKC_SHIFT: u32 = crate::tables::CCC_QC_NFKC_SHIFT;
/// See [`CCC_QC_NFC_SHIFT`].
pub const CCC_QC_NFKD_SHIFT: u32 = crate::tables::CCC_QC_NFKD_SHIFT;

/// Look up `(ccc, qc_bits)` for a character under the given form shift.
/// `qc_bits` is 0 for `Yes`, nonzero for `No`/`Maybe`. See
/// `src/tables/mod.rs:409` for the authoritative definition.
#[inline]
pub fn lookup_ccc_qc(c: char, qc_shift: u32) -> (u8, u8) {
    crate::tables::lookup_ccc_qc(c, qc_shift)
}
