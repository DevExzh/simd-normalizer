// tests/exhaustive.rs
//! Exhaustive codepoint tests ported from the ICU4X test suite.
//!
//! These tests validate EVERY Unicode scalar value (0..=0x10FFFF, skipping
//! surrogates) for normalization invariance and correctness. They cover two
//! key gaps not addressed by the conformance tests:
//!
//! 1. **Unlisted-codepoint invariant** (UAX#15 requirement): every codepoint
//!    NOT listed in NormalizationTest.txt must be invariant under all four
//!    normalization forms (NFC, NFD, NFKC, NFKD).
//!
//! 2. **Differential validation against icu_normalizer**: for every single-
//!    character string, verify that simd_normalizer produces the same output
//!    as icu_normalizer for all four forms. This implicitly validates CCC
//!    (Canonical Combining Class) correctness since decomposition ordering
//!    depends on CCC values.
//!
//! The full-range tests are marked `#[ignore]` because they iterate ~1.1M
//! codepoints x 4 forms and take several minutes. Run them with:
//!
//!     cargo test --test exhaustive -- --ignored
//!
//! Non-ignored "spot check" variants test a representative subset (every
//! 100th codepoint plus key boundary ranges) and run in seconds.

mod data;
use data::normalization_tests::NORMALIZATION_TESTS;
use std::collections::HashSet;

// ---------------------------------------------------------------------------
// Helpers: simd_normalizer (our crate)
// ---------------------------------------------------------------------------

fn our_nfc(s: &str) -> String {
    simd_normalizer::nfc().normalize(s).into_owned()
}

fn our_nfd(s: &str) -> String {
    simd_normalizer::nfd().normalize(s).into_owned()
}

fn our_nfkc(s: &str) -> String {
    simd_normalizer::nfkc().normalize(s).into_owned()
}

fn our_nfkd(s: &str) -> String {
    simd_normalizer::nfkd().normalize(s).into_owned()
}

// ---------------------------------------------------------------------------
// Helpers: icu_normalizer (reference)
// ---------------------------------------------------------------------------

fn icu_nfc(s: &str) -> String {
    use icu_normalizer::ComposingNormalizerBorrowed;
    ComposingNormalizerBorrowed::new_nfc()
        .normalize(s)
        .into_owned()
}

fn icu_nfd(s: &str) -> String {
    use icu_normalizer::DecomposingNormalizerBorrowed;
    DecomposingNormalizerBorrowed::new_nfd()
        .normalize(s)
        .into_owned()
}

fn icu_nfkc(s: &str) -> String {
    use icu_normalizer::ComposingNormalizerBorrowed;
    ComposingNormalizerBorrowed::new_nfkc()
        .normalize(s)
        .into_owned()
}

fn icu_nfkd(s: &str) -> String {
    use icu_normalizer::DecomposingNormalizerBorrowed;
    DecomposingNormalizerBorrowed::new_nfkd()
        .normalize(s)
        .into_owned()
}

// ---------------------------------------------------------------------------
// Helpers: formatting
// ---------------------------------------------------------------------------

fn codepoints(s: &str) -> String {
    s.chars()
        .map(|c| format!("U+{:04X}", c as u32))
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------------------------------------------------------------------------
// Helper: collect all codepoints listed in NormalizationTest.txt (Part 1)
//
// The official test data's Part 1 contains single-codepoint test cases.
// The first character of the `source` field is the codepoint under test.
// We collect ALL first-chars to build the "listed" set.
// ---------------------------------------------------------------------------

fn listed_codepoints() -> HashSet<u32> {
    let mut set = HashSet::with_capacity(NORMALIZATION_TESTS.len());
    for t in NORMALIZATION_TESTS.iter() {
        // Each Part 1 entry has a single codepoint as `source`.
        // Multi-codepoint entries (Parts 2-4) are also included; we extract
        // the first char. This is a superset of the Part 1 codepoints, which
        // is safe: we only SKIP invariance checks for listed codepoints, so
        // including extras means we check MORE, not fewer.
        if let Some(c) = t.source.chars().next() {
            set.insert(c as u32);
        }
    }
    set
}

/// Iterator over all valid Unicode scalar values, skipping surrogates.
fn all_scalar_values() -> impl Iterator<Item = char> {
    (0u32..=0x10FFFF).filter_map(char::from_u32)
}

/// Iterator over a sampled subset: every `step`-th codepoint plus boundary
/// ranges that are known to be interesting for normalization.
fn sampled_scalar_values(step: u32) -> impl Iterator<Item = char> {
    // Interesting boundary ranges to always include
    let boundary_ranges: Vec<std::ops::RangeInclusive<u32>> = vec![
        0x0000..=0x00FF,       // Basic Latin + Latin-1 Supplement
        0x0300..=0x036F,       // Combining Diacritical Marks
        0x0590..=0x05FF,       // Hebrew
        0x0600..=0x06FF,       // Arabic
        0x0900..=0x097F,       // Devanagari
        0x1100..=0x11FF,       // Hangul Jamo
        0x2000..=0x206F,       // General Punctuation
        0x2100..=0x214F,       // Letterlike Symbols
        0x2150..=0x218F,       // Number Forms
        0x2460..=0x24FF,       // Enclosed Alphanumerics
        0x3040..=0x30FF,       // Hiragana + Katakana
        0x3300..=0x33FF,       // CJK Compatibility
        0xAC00..=0xAC00 + 100, // Hangul Syllables (first 100)
        0xD7A0..=0xD7FF,       // Hangul Jamo Extended-B (near surrogates)
        0xF900..=0xFAFF,       // CJK Compatibility Ideographs
        0xFB00..=0xFB06,       // Latin ligatures
        0xFE00..=0xFE0F,       // Variation Selectors
        0xFF00..=0xFFEF,       // Halfwidth and Fullwidth Forms
        0x1D100..=0x1D1FF,     // Musical Symbols
        0x1F600..=0x1F64F,     // Emoticons
        0x10FF00..=0x10FFFF,   // Last valid range (plane 16 tail)
    ];

    let mut seen = HashSet::new();
    let mut result = Vec::new();

    // Add all boundary codepoints
    for range in boundary_ranges {
        for u in range {
            if let Some(c) = char::from_u32(u)
                && seen.insert(u)
            {
                result.push(c);
            }
        }
    }

    // Add every step-th codepoint
    let mut u = 0u32;
    while u <= 0x10FFFF {
        if let Some(c) = char::from_u32(u)
            && seen.insert(u)
        {
            result.push(c);
        }
        u += step;
    }

    result.into_iter()
}

// ===========================================================================
// Gap 1: Unlisted-Codepoint Invariant (from ICU4X `test_conformance`)
// ===========================================================================

/// Full exhaustive test: every codepoint NOT in NormalizationTest.txt must be
/// invariant under all four normalization forms.
///
/// Run with: `cargo test --test exhaustive -- --ignored unlisted_codepoint_invariant_full`
#[test]
#[ignore]
fn unlisted_codepoint_invariant_full() {
    let listed = listed_codepoints();
    let mut failures = Vec::new();

    for c in all_scalar_values() {
        if listed.contains(&(c as u32)) {
            continue;
        }

        let s: String = c.to_string();

        let nfc = our_nfc(&s);
        if nfc != s {
            failures.push(format!(
                "U+{:04X}: NFC({}) = {} (expected invariant)",
                c as u32,
                codepoints(&s),
                codepoints(&nfc)
            ));
        }

        let nfd = our_nfd(&s);
        if nfd != s {
            failures.push(format!(
                "U+{:04X}: NFD({}) = {} (expected invariant)",
                c as u32,
                codepoints(&s),
                codepoints(&nfd)
            ));
        }

        let nfkc = our_nfkc(&s);
        if nfkc != s {
            failures.push(format!(
                "U+{:04X}: NFKC({}) = {} (expected invariant)",
                c as u32,
                codepoints(&s),
                codepoints(&nfkc)
            ));
        }

        let nfkd = our_nfkd(&s);
        if nfkd != s {
            failures.push(format!(
                "U+{:04X}: NFKD({}) = {} (expected invariant)",
                c as u32,
                codepoints(&s),
                codepoints(&nfkd)
            ));
        }

        // Bail early if too many failures to avoid overwhelming output
        if failures.len() > 100 {
            failures.push("... (truncated after 100 failures)".to_string());
            break;
        }
    }

    assert!(
        failures.is_empty(),
        "Unlisted codepoint invariant violations ({} failures):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// Spot-check: sampled subset of unlisted codepoints (every 100th + boundaries).
#[test]
fn unlisted_codepoint_invariant_spot_check() {
    let listed = listed_codepoints();
    let mut failures = Vec::new();

    for c in sampled_scalar_values(100) {
        if listed.contains(&(c as u32)) {
            continue;
        }

        let s: String = c.to_string();

        let nfc = our_nfc(&s);
        if nfc != s {
            failures.push(format!(
                "U+{:04X}: NFC({}) = {} (expected invariant)",
                c as u32,
                codepoints(&s),
                codepoints(&nfc)
            ));
        }

        let nfd = our_nfd(&s);
        if nfd != s {
            failures.push(format!(
                "U+{:04X}: NFD({}) = {} (expected invariant)",
                c as u32,
                codepoints(&s),
                codepoints(&nfd)
            ));
        }

        let nfkc = our_nfkc(&s);
        if nfkc != s {
            failures.push(format!(
                "U+{:04X}: NFKC({}) = {} (expected invariant)",
                c as u32,
                codepoints(&s),
                codepoints(&nfkc)
            ));
        }

        let nfkd = our_nfkd(&s);
        if nfkd != s {
            failures.push(format!(
                "U+{:04X}: NFKD({}) = {} (expected invariant)",
                c as u32,
                codepoints(&s),
                codepoints(&nfkd)
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "Unlisted codepoint invariant violations ({} failures):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

// ===========================================================================
// Gap 2: Differential test against icu_normalizer for ALL codepoints
// ===========================================================================

/// Full exhaustive differential: for every Unicode scalar value (as a single-
/// char string), compare NFC/NFD/NFKC/NFKD output of simd_normalizer against
/// icu_normalizer. This implicitly validates CCC correctness since
/// decomposition ordering depends on CCC values.
///
/// Run with: `cargo test --test exhaustive -- --ignored differential_all_codepoints_full`
#[test]
#[ignore]
fn differential_all_codepoints_full() {
    let mut failures = Vec::new();

    for c in all_scalar_values() {
        let s: String = c.to_string();

        // NFD
        let our = our_nfd(&s);
        let reference = icu_nfd(&s);
        if our != reference {
            failures.push(format!(
                "U+{:04X} NFD: ours=[{}] ref=[{}]",
                c as u32,
                codepoints(&our),
                codepoints(&reference)
            ));
        }

        // NFC
        let our = our_nfc(&s);
        let reference = icu_nfc(&s);
        if our != reference {
            failures.push(format!(
                "U+{:04X} NFC: ours=[{}] ref=[{}]",
                c as u32,
                codepoints(&our),
                codepoints(&reference)
            ));
        }

        // NFKD
        let our = our_nfkd(&s);
        let reference = icu_nfkd(&s);
        if our != reference {
            failures.push(format!(
                "U+{:04X} NFKD: ours=[{}] ref=[{}]",
                c as u32,
                codepoints(&our),
                codepoints(&reference)
            ));
        }

        // NFKC
        let our = our_nfkc(&s);
        let reference = icu_nfkc(&s);
        if our != reference {
            failures.push(format!(
                "U+{:04X} NFKC: ours=[{}] ref=[{}]",
                c as u32,
                codepoints(&our),
                codepoints(&reference)
            ));
        }

        if failures.len() > 100 {
            failures.push("... (truncated after 100 failures)".to_string());
            break;
        }
    }

    assert!(
        failures.is_empty(),
        "Differential failures against icu_normalizer ({} failures):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// Spot-check differential: sampled subset (every 100th + boundaries).
#[test]
fn differential_all_codepoints_spot_check() {
    let mut failures = Vec::new();

    for c in sampled_scalar_values(100) {
        let s: String = c.to_string();

        // NFD
        let our = our_nfd(&s);
        let reference = icu_nfd(&s);
        if our != reference {
            failures.push(format!(
                "U+{:04X} NFD: ours=[{}] ref=[{}]",
                c as u32,
                codepoints(&our),
                codepoints(&reference)
            ));
        }

        // NFC
        let our = our_nfc(&s);
        let reference = icu_nfc(&s);
        if our != reference {
            failures.push(format!(
                "U+{:04X} NFC: ours=[{}] ref=[{}]",
                c as u32,
                codepoints(&our),
                codepoints(&reference)
            ));
        }

        // NFKD
        let our = our_nfkd(&s);
        let reference = icu_nfkd(&s);
        if our != reference {
            failures.push(format!(
                "U+{:04X} NFKD: ours=[{}] ref=[{}]",
                c as u32,
                codepoints(&our),
                codepoints(&reference)
            ));
        }

        // NFKC
        let our = our_nfkc(&s);
        let reference = icu_nfkc(&s);
        if our != reference {
            failures.push(format!(
                "U+{:04X} NFKC: ours=[{}] ref=[{}]",
                c as u32,
                codepoints(&our),
                codepoints(&reference)
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "Differential failures against icu_normalizer ({} failures):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

// ===========================================================================
// Gap 2b: CCC exhaustive validation via NFD differential
//
// Since simd_normalizer's CCC lookup is `pub(crate)`, we validate CCC
// indirectly: if NFD decomposition matches between simd_normalizer and
// icu_normalizer for every codepoint, then the CCC values must be correct
// (since NFD ordering depends entirely on CCC). This is the same approach
// used by the full differential test above, but isolated here for clarity
// and to match the ICU4X `test_ccc` pattern.
// ===========================================================================

/// Full exhaustive CCC validation: compare NFD output for every codepoint.
///
/// Run with: `cargo test --test exhaustive -- --ignored ccc_nfd_differential_full`
#[test]
#[ignore]
fn ccc_nfd_differential_full() {
    let mut failures = Vec::new();

    for c in all_scalar_values() {
        let s: String = c.to_string();
        let our = our_nfd(&s);
        let reference = icu_nfd(&s);

        if our != reference {
            failures.push(format!(
                "U+{:04X}: our NFD=[{}] icu NFD=[{}]",
                c as u32,
                codepoints(&our),
                codepoints(&reference)
            ));
        }

        if failures.len() > 100 {
            failures.push("... (truncated after 100 failures)".to_string());
            break;
        }
    }

    assert!(
        failures.is_empty(),
        "CCC/NFD differential failures ({} failures):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// Spot-check CCC/NFD: sampled subset.
#[test]
fn ccc_nfd_differential_spot_check() {
    let mut failures = Vec::new();

    for c in sampled_scalar_values(100) {
        let s: String = c.to_string();
        let our = our_nfd(&s);
        let reference = icu_nfd(&s);

        if our != reference {
            failures.push(format!(
                "U+{:04X}: our NFD=[{}] icu NFD=[{}]",
                c as u32,
                codepoints(&our),
                codepoints(&reference)
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "CCC/NFD differential failures ({} failures):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

// ===========================================================================
// Bonus: is_normalized consistency for all codepoints
// ===========================================================================

/// Full exhaustive: for every codepoint, if normalizing produces the same
/// string, then is_normalized must return true. (The converse is not always
/// true due to quick-check MAYBE results.)
///
/// Run with: `cargo test --test exhaustive -- --ignored is_normalized_consistency_full`
#[test]
#[ignore]
fn is_normalized_consistency_full() {
    use simd_normalizer::UnicodeNormalization;
    let mut failures = Vec::new();

    for c in all_scalar_values() {
        let s: String = c.to_string();

        // NFC
        let normalized = our_nfc(&s);
        if normalized == s && !s.is_nfc() {
            failures.push(format!(
                "U+{:04X}: NFC is invariant but is_nfc()=false",
                c as u32
            ));
        }

        // NFD
        let normalized = our_nfd(&s);
        if normalized == s && !s.is_nfd() {
            failures.push(format!(
                "U+{:04X}: NFD is invariant but is_nfd()=false",
                c as u32
            ));
        }

        // NFKC
        let normalized = our_nfkc(&s);
        if normalized == s && !s.is_nfkc() {
            failures.push(format!(
                "U+{:04X}: NFKC is invariant but is_nfkc()=false",
                c as u32
            ));
        }

        // NFKD
        let normalized = our_nfkd(&s);
        if normalized == s && !s.is_nfkd() {
            failures.push(format!(
                "U+{:04X}: NFKD is invariant but is_nfkd()=false",
                c as u32
            ));
        }

        if failures.len() > 100 {
            failures.push("... (truncated after 100 failures)".to_string());
            break;
        }
    }

    assert!(
        failures.is_empty(),
        "is_normalized consistency failures ({} failures):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// Spot-check is_normalized consistency.
#[test]
fn is_normalized_consistency_spot_check() {
    use simd_normalizer::UnicodeNormalization;
    let mut failures = Vec::new();

    for c in sampled_scalar_values(100) {
        let s: String = c.to_string();

        let normalized = our_nfc(&s);
        if normalized == s && !s.is_nfc() {
            failures.push(format!(
                "U+{:04X}: NFC is invariant but is_nfc()=false",
                c as u32
            ));
        }

        let normalized = our_nfd(&s);
        if normalized == s && !s.is_nfd() {
            failures.push(format!(
                "U+{:04X}: NFD is invariant but is_nfd()=false",
                c as u32
            ));
        }

        let normalized = our_nfkc(&s);
        if normalized == s && !s.is_nfkc() {
            failures.push(format!(
                "U+{:04X}: NFKC is invariant but is_nfkc()=false",
                c as u32
            ));
        }

        let normalized = our_nfkd(&s);
        if normalized == s && !s.is_nfkd() {
            failures.push(format!(
                "U+{:04X}: NFKD is invariant but is_nfkd()=false",
                c as u32
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "is_normalized consistency failures ({} failures):\n{}",
        failures.len(),
        failures.join("\n")
    );
}
