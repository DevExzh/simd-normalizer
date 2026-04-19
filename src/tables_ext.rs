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
