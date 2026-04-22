//! Panic-safety sweeps for every public `&str`-taking entry point.
//!
//! Closes the gap identified in
//! `docs/superpowers/specs/2026-04-17-full-edge-case-coverage-design.md`
//! section 3: for every Unicode scalar, each public entry point must
//! return without panic and (where applicable) produce well-formed output.
//!
//! The default fast tests use a stride of 0x100 (~4,400 scalars). The
//! full-sweep tests iterate every valid scalar and are marked `#[ignore]`,
//! matching the convention in `tests/exhaustive.rs`.

use simd_normalizer::matching::{
    MatchingOptions, matches_normalized, normalize_for_matching, normalize_for_matching_utf16,
};
use simd_normalizer::{
    CaseFoldMode, IsNormalized, UnicodeNormalization, are_confusable, casefold, casefold_char, nfc,
    nfd, nfkc, nfkd, skeleton,
};

/// Iterator over valid Unicode scalars in `0..=0x10FFFF`, skipping surrogates.
fn all_scalars() -> impl Iterator<Item = char> {
    (0u32..=0x10FFFF).filter_map(char::from_u32)
}

/// Spot-check iterator: every 0x100-th valid scalar.
fn sampled_scalars() -> impl Iterator<Item = char> {
    (0u32..=0x10FFFF).step_by(0x100).filter_map(char::from_u32)
}

// ---------------------------------------------------------------------------
// Normalization forms (spot-check)
// ---------------------------------------------------------------------------

#[test]
fn panic_safety_nfc_all_scalars_spot_check() {
    for c in sampled_scalars() {
        let s: String = c.to_string();
        // Free-fn path
        let _ = nfc().normalize(&s);
        let _ = nfc().is_normalized(&s);
        let _ = nfc().quick_check(&s);
        // Trait path
        let _ = s.as_str().nfc();
        let _ = s.as_str().is_nfc();
    }
}

#[test]
fn panic_safety_nfd_all_scalars_spot_check() {
    for c in sampled_scalars() {
        let s: String = c.to_string();
        let _ = nfd().normalize(&s);
        let _ = nfd().is_normalized(&s);
        let _ = nfd().quick_check(&s);
        let _ = s.as_str().nfd();
        let _ = s.as_str().is_nfd();
    }
}

#[test]
fn panic_safety_nfkc_all_scalars_spot_check() {
    for c in sampled_scalars() {
        let s: String = c.to_string();
        let _ = nfkc().normalize(&s);
        let _ = nfkc().is_normalized(&s);
        let _ = nfkc().quick_check(&s);
        let _ = s.as_str().nfkc();
        let _ = s.as_str().is_nfkc();
    }
}

#[test]
fn panic_safety_nfkd_all_scalars_spot_check() {
    for c in sampled_scalars() {
        let s: String = c.to_string();
        let _ = nfkd().normalize(&s);
        let _ = nfkd().is_normalized(&s);
        let _ = nfkd().quick_check(&s);
        let _ = s.as_str().nfkd();
        let _ = s.as_str().is_nfkd();
    }
}

// ---------------------------------------------------------------------------
// Casefold (string and char) — spot-check
// ---------------------------------------------------------------------------

#[test]
fn panic_safety_casefold_standard_all_scalars_spot_check() {
    for c in sampled_scalars() {
        let s: String = c.to_string();
        let _ = casefold(&s, CaseFoldMode::Standard);
        let _ = casefold_char(c, CaseFoldMode::Standard);
    }
}

#[test]
fn panic_safety_casefold_turkish_all_scalars_spot_check() {
    for c in sampled_scalars() {
        let s: String = c.to_string();
        let _ = casefold(&s, CaseFoldMode::Turkish);
        let _ = casefold_char(c, CaseFoldMode::Turkish);
    }
}

// ---------------------------------------------------------------------------
// Skeleton / are_confusable — spot-check
// ---------------------------------------------------------------------------

#[test]
fn panic_safety_skeleton_all_scalars_spot_check() {
    for c in sampled_scalars() {
        let s: String = c.to_string();
        let _ = skeleton(&s);
    }
}

#[test]
fn panic_safety_are_confusable_reflexive_all_scalars_spot_check() {
    for c in sampled_scalars() {
        let s: String = c.to_string();
        let _ = are_confusable(&s, &s);
    }
}

// ---------------------------------------------------------------------------
// Matching pipeline — spot-check
// ---------------------------------------------------------------------------

#[test]
fn panic_safety_normalize_for_matching_all_scalars_spot_check() {
    let opts = MatchingOptions::default();
    for c in sampled_scalars() {
        let s: String = c.to_string();
        let _ = normalize_for_matching(&s, &opts);
    }
}

#[test]
fn panic_safety_normalize_for_matching_utf16_all_scalars_spot_check() {
    let opts = MatchingOptions::default();
    for c in sampled_scalars() {
        let s: String = c.to_string();
        let out = normalize_for_matching_utf16(&s, &opts);
        // Must decode back to a valid String (no lone surrogates).
        String::from_utf16(&out).unwrap_or_else(|_| {
            panic!(
                "normalize_for_matching_utf16 produced ill-formed UTF-16 for scalar U+{:04X}",
                c as u32
            )
        });
    }
}

#[test]
fn panic_safety_matches_normalized_reflexive_all_scalars_spot_check() {
    let opts = MatchingOptions::default();
    for c in sampled_scalars() {
        let s: String = c.to_string();
        let _ = matches_normalized(&s, &s, &opts);
    }
}

#[test]
fn panic_safety_quick_check_all_forms_all_scalars_spot_check() {
    for c in sampled_scalars() {
        let s: String = c.to_string();
        // quick_check must produce a valid IsNormalized discriminant.
        let qc = nfc().quick_check(&s);
        let _ = matches!(
            qc,
            IsNormalized::Yes | IsNormalized::No | IsNormalized::Maybe
        );
        let _ = nfd().quick_check(&s);
        let _ = nfkc().quick_check(&s);
        let _ = nfkd().quick_check(&s);
    }
}

// ---------------------------------------------------------------------------
// Full sweeps (#[ignore]) — iterate every valid scalar.
// Run with: `cargo test --test panic_safety_sweeps -- --ignored`.
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn panic_safety_all_forms_full_sweep() {
    for c in all_scalars() {
        let s: String = c.to_string();
        let _ = nfc().normalize(&s);
        let _ = nfd().normalize(&s);
        let _ = nfkc().normalize(&s);
        let _ = nfkd().normalize(&s);
    }
}

#[test]
#[ignore]
fn panic_safety_casefold_skeleton_full_sweep() {
    for c in all_scalars() {
        let s: String = c.to_string();
        let _ = casefold(&s, CaseFoldMode::Standard);
        let _ = casefold(&s, CaseFoldMode::Turkish);
        let _ = casefold_char(c, CaseFoldMode::Standard);
        let _ = casefold_char(c, CaseFoldMode::Turkish);
        let _ = skeleton(&s);
    }
}

#[test]
#[ignore]
fn panic_safety_matching_full_sweep() {
    let opts = MatchingOptions::default();
    for c in all_scalars() {
        let s: String = c.to_string();
        let _ = normalize_for_matching(&s, &opts);
        let out = normalize_for_matching_utf16(&s, &opts);
        String::from_utf16(&out)
            .unwrap_or_else(|_| panic!("ill-formed UTF-16 for scalar U+{:04X}", c as u32));
        let _ = matches_normalized(&s, &s, &opts);
        let _ = are_confusable(&s, &s);
    }
}
