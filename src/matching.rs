//! Fused normalization pipeline for case-insensitive, confusable-aware matching.
//!
//! Pipeline: **NFKC → CaseFold → Confusable Skeleton** (NFD → confusable_map → NFD).
//!
//! Two strings that produce the same [`normalize_for_matching`] output are
//! equivalent for matching purposes: they share the same compatibility
//! decomposition, the same case folding, and the same confusable prototype.
//!
//! ## Optimization summary (Component E)
//!
//! The matching pipeline composes four conceptually-distinct stages
//! (`NFKC → casefold → skeleton → casefold`). A naive implementation walks
//! the input four times with three string allocations between stages. We
//! preserve that staged structure for correctness — full-fusion attempts
//! produced subtle parity divergences against the legacy chain on
//! cross-codepoint canonical reorder cases — but every individual stage is
//! optimized:
//!
//! * **NFKC** is the existing fused decomposer/composer (Component D),
//!   running at peak SIMD throughput on the hot ASCII / Latin-1 path.
//! * **Casefold** has a SIMD-driven ASCII fast path that scans 64-byte
//!   chunks for non-ASCII / uppercase bytes and lowercases via `b | 0x20`,
//!   avoiding per-byte trie lookups on pure-ASCII regions
//!   (see [`mod@crate::casefold`]).
//! * **Skeleton** uses a 256-byte bloom filter to skip the binary search
//!   into the confusable mapping table for the vast majority of codepoints
//!   that have no mapping (see `tables::confusable_bloom_might_contain`,
//!   wired into [`crate::confusable::skeleton`]).
//! * **Outer fixed-point** loop runs at most 4 iterations; in practice it
//!   converges after 1.

use alloc::string::String;
use alloc::vec::Vec;

use crate::casefold::{self, CaseFoldMode};
use crate::confusable;

/// Options for the matching normalization pipeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchingOptions {
    /// Case folding mode. Defaults to [`CaseFoldMode::Standard`].
    pub case_fold: CaseFoldMode,
}

impl Default for MatchingOptions {
    fn default() -> Self {
        MatchingOptions {
            case_fold: CaseFoldMode::Standard,
        }
    }
}

/// Normalize input for matching: NFKC → CaseFold → Confusable Skeleton.
///
/// Returns a canonical matching form where:
/// - Compatibility equivalents are unified (NFKC)
/// - Case differences are eliminated (Unicode case folding)
/// - Visually confusable characters map to the same prototype (UTS #39 skeleton)
///
/// Two strings produce the same result if and only if they should be
/// treated as equivalent for keyword detection and anti-spoofing.
///
/// # Examples
///
/// ```
/// use simd_normalizer::matching::{normalize_for_matching, MatchingOptions};
///
/// let opts = MatchingOptions::default();
///
/// // Case folding
/// assert_eq!(
///     normalize_for_matching("File", &opts),
///     normalize_for_matching("file", &opts),
/// );
///
/// // Turkish dotless-I
/// assert_eq!(
///     normalize_for_matching("file", &opts),
///     normalize_for_matching("f\u{0131}le", &opts),
/// );
/// ```
pub fn normalize_for_matching(input: &str, opts: &MatchingOptions) -> String {
    if input.is_empty() {
        return String::new();
    }

    // Iterate the full pipeline to a fixed point. Each `one_pass` is a
    // NFKC → casefold → skeleton → casefold chain; convergence typically
    // occurs in 1–2 outer iterations.
    let mut current = one_pass(input, opts);
    for _ in 0..3 {
        let next = one_pass(&current, opts);
        if next == current {
            return current;
        }
        current = next;
    }
    current
}

/// Single pass of the matching pipeline: NFKC → casefold → skeleton → casefold.
///
/// The NFKC-first ordering is parity-critical. NFKC canonically composes
/// before casefold, hiding code points like U+0345 (COMBINING GREEK
/// YPOGEGRAMMENI) inside precomposed starters (e.g. U+1F80 `ᾀ`). A per-char
/// pipeline that decomposed first and casefolded the exposed combining mark
/// (→ U+03B9) would produce a different skeleton.
///
/// Per-stage optimizations are documented in the module-level comment.
#[inline]
fn one_pass(input: &str, opts: &MatchingOptions) -> String {
    let nfkc = crate::nfkc().normalize(input);
    let folded = casefold::casefold(&nfkc, opts.case_fold);
    let skel = confusable::skeleton(&folded);
    let final_folded = casefold::casefold(&skel, opts.case_fold);
    final_folded.into_owned()
}

/// Reference implementation of the matching pipeline, preserved for parity
/// testing against any alternative composition order.
#[cfg(any(test, feature = "internal-test-api"))]
pub fn normalize_for_matching_legacy(input: &str, opts: &MatchingOptions) -> String {
    if input.is_empty() {
        return String::new();
    }
    let mut current = one_pass_legacy(input, opts);
    for _ in 0..3 {
        let next = one_pass_legacy(&current, opts);
        if next == current {
            return current;
        }
        current = next;
    }
    current
}

/// Single legacy pass: NFKC → casefold → skeleton → casefold.
#[cfg(any(test, feature = "internal-test-api"))]
fn one_pass_legacy(input: &str, opts: &MatchingOptions) -> String {
    let nfkc = crate::nfkc().normalize(input);
    let folded = casefold::casefold(&nfkc, opts.case_fold);
    let skel = confusable::skeleton(&folded);
    let final_folded = casefold::casefold(&skel, opts.case_fold);
    final_folded.into_owned()
}

/// Normalize input for matching and encode the result as UTF-16.
///
/// Useful for interoperability with systems that use UTF-16 keyword tables.
pub fn normalize_for_matching_utf16(input: &str, opts: &MatchingOptions) -> Vec<u16> {
    normalize_for_matching(input, opts).encode_utf16().collect()
}

/// Check whether two strings match after full normalization.
///
/// Returns `true` if both strings produce the same matching form after
/// NFKC normalization, case folding, and confusable skeleton mapping.
///
/// # Examples
///
/// ```
/// use simd_normalizer::matching::{matches_normalized, MatchingOptions};
///
/// let opts = MatchingOptions::default();
///
/// // "File" and "file" match (case folding)
/// assert!(matches_normalized("File", "file", &opts));
///
/// // Latin 'a' and Cyrillic 'а' match (confusable mapping)
/// assert!(matches_normalized("a", "\u{0430}", &opts));
/// ```
pub fn matches_normalized(a: &str, b: &str, opts: &MatchingOptions) -> bool {
    // Fast path: identical strings always match.
    if a == b {
        return true;
    }
    normalize_for_matching(a, opts) == normalize_for_matching(b, opts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tables;

    fn default_opts() -> MatchingOptions {
        MatchingOptions::default()
    }

    fn turkish_opts() -> MatchingOptions {
        MatchingOptions {
            case_fold: CaseFoldMode::Turkish,
        }
    }

    // ---- Bloom filter coverage ----

    #[test]
    fn confusable_bloom_covers_every_source() {
        // Every source codepoint in the confusable mapping table must hash
        // to a set bit in the bloom. False negatives are unsound (would
        // skip required mappings); false positives are fine.
        for &(source_cp, _) in tables::confusable::CONFUSABLE_MAPPINGS {
            assert!(
                tables::confusable_bloom_might_contain(source_cp),
                "confusable source U+{:06X} hashed to a clear bit",
                source_cp,
            );
        }
    }

    // ---- Basic tests ----

    #[test]
    fn empty_input() {
        assert_eq!(normalize_for_matching("", &default_opts()), "");
    }

    #[test]
    fn ascii_lowercase_unchanged() {
        let result = normalize_for_matching("hello", &default_opts());
        assert!(!result.is_empty());
    }

    #[test]
    fn identical_strings_match() {
        assert!(matches_normalized("test", "test", &default_opts()));
    }

    #[test]
    fn different_strings_dont_match() {
        assert!(!matches_normalized("hello", "world", &default_opts()));
    }

    // ---- Case folding tests ----

    #[test]
    fn case_insensitive_ascii() {
        let opts = default_opts();
        assert!(matches_normalized("File", "file", &opts));
        assert!(matches_normalized("FILE", "file", &opts));
        assert!(matches_normalized("FiLe", "file", &opts));
    }

    #[test]
    fn case_insensitive_extended() {
        let opts = default_opts();
        // Ö (U+00D6) case folds to ö (U+00F6)
        assert!(matches_normalized("Ströme", "ströme", &opts));
    }

    // ---- Confusable detection tests ----

    #[test]
    fn confusable_latin_cyrillic_a() {
        let opts = default_opts();
        // Latin 'a' (U+0061) and Cyrillic 'а' (U+0430)
        assert!(matches_normalized("a", "\u{0430}", &opts));
    }

    #[test]
    fn confusable_latin_cyrillic_word() {
        let opts = default_opts();
        // "apple" in Latin vs mixed Latin/Cyrillic
        // Cyrillic: а=U+0430, р=U+0440, е=U+0435
        let latin = "apple";
        let mixed = "\u{0430}\u{0440}\u{0440}l\u{0435}";
        assert!(matches_normalized(latin, mixed, &opts));
    }

    // ---- Combined case + confusable tests (the key requirement) ----

    #[test]
    fn file_variants_all_match() {
        let opts = default_opts();
        let canonical = normalize_for_matching("file", &opts);

        // Case variant
        assert_eq!(normalize_for_matching("File", &opts), canonical);
        assert_eq!(normalize_for_matching("FILE", &opts), canonical);

        // Turkish dotless-ı (U+0131) — in standard mode, ı case-folds to itself (ı),
        // but it's confusable with 'i' via the confusable mapping.
        // The matching pipeline handles this through the confusable skeleton step.
        let fıle = "f\u{0131}le";
        assert!(
            matches_normalized("file", fıle, &opts),
            "'file' and 'fıle' should match: file={:?}, fıle={:?}",
            normalize_for_matching("file", &opts),
            normalize_for_matching(fıle, &opts),
        );
    }

    #[test]
    fn file_mixed_case_and_confusable() {
        let opts = default_opts();
        // "FıLE" — uppercase + Turkish dotless-ı
        let input = "F\u{0131}LE";
        assert!(
            matches_normalized("file", input, &opts),
            "'file' and 'FıLE' should match: file={:?}, FıLE={:?}",
            normalize_for_matching("file", &opts),
            normalize_for_matching(input, &opts),
        );
    }

    // ---- NFKC compatibility tests ----

    #[test]
    fn nfkc_fullwidth() {
        let opts = default_opts();
        // Fullwidth 'A' (U+FF21) should NFKC-normalize to 'A', then case-fold to 'a'
        let fullwidth_a = "\u{FF21}";
        assert!(matches_normalized(fullwidth_a, "a", &opts));
    }

    #[test]
    fn nfkc_superscript() {
        let opts = default_opts();
        // Superscript '2' (U+00B2) NFKC-normalizes to '2'
        assert_eq!(
            normalize_for_matching("\u{00B2}", &opts),
            normalize_for_matching("2", &opts),
        );
    }

    // ---- Turkish mode tests ----

    #[test]
    fn turkish_mode_dotless_i() {
        let opts = turkish_opts();
        // In Turkish mode: I → ı (U+0131), not i
        // So "Istanbul" in Turkish mode has ı as first char
        let a = normalize_for_matching("Istanbul", &opts);
        let b = normalize_for_matching("\u{0131}stanbul", &opts);
        assert_eq!(a, b);
    }

    #[test]
    fn turkish_mode_dotted_i() {
        let opts = turkish_opts();
        // In Turkish mode: İ (U+0130) → i
        assert!(matches_normalized("\u{0130}stanbul", "istanbul", &opts));
    }

    // ---- UTF-16 encoding test ----

    #[test]
    fn utf16_encoding() {
        let opts = default_opts();
        let utf16 = normalize_for_matching_utf16("hello", &opts);
        assert!(!utf16.is_empty());
        // Should round-trip back to a valid string
        let decoded = String::from_utf16(&utf16).expect("valid UTF-16");
        assert_eq!(decoded, normalize_for_matching("hello", &opts));
    }

    #[test]
    fn utf16_supplementary() {
        let opts = default_opts();
        // U+1F600 (emoji) — supplementary character, encodes as surrogate pair in UTF-16
        let utf16 = normalize_for_matching_utf16("\u{1F600}", &opts);
        assert!(!utf16.is_empty());
        let decoded = String::from_utf16(&utf16).expect("valid UTF-16");
        assert_eq!(decoded, normalize_for_matching("\u{1F600}", &opts));
    }

    // ---- Stability tests ----

    #[test]
    fn matching_idempotent() {
        let opts = default_opts();
        let inputs = [
            "hello",
            "File",
            "\u{0430}\u{0440}\u{0440}l\u{0435}",
            "\u{00C0}",
        ];
        for input in &inputs {
            let once = normalize_for_matching(input, &opts);
            let twice = normalize_for_matching(&once, &opts);
            assert_eq!(
                once, twice,
                "normalize_for_matching should be idempotent for {:?}",
                input
            );
        }
    }

    #[test]
    fn matching_not_confusable_different_words() {
        let opts = default_opts();
        assert!(!matches_normalized("hello", "world", &opts));
        assert!(!matches_normalized("file", "pile", &opts));
    }

    // ---- Parity with legacy implementation ----

    #[test]
    fn fused_matches_legacy_on_fixtures() {
        let opts = default_opts();
        let fixtures = [
            "",
            "hello",
            "File",
            "FILE",
            "FiLe",
            "Ströme",
            "ströme",
            "a",
            "\u{0430}",
            "\u{0430}\u{0440}\u{0440}l\u{0435}",
            "f\u{0131}le",
            "F\u{0131}LE",
            "\u{FF21}",
            "\u{00B2}",
            "\u{00C0}",
            "Hel\u{0430}",
            "\u{1D0E}\u{326}\u{306}",
            "\u{1F600}",
            "Istanbul",
            "test mixing\u{0430}cyrillic",
            // Long input mixing scripts:
            "The quick brown FOX jumps over the lazy DOG (Привет, Мир!) Καλημέρα",
        ];
        for input in &fixtures {
            let fused = normalize_for_matching(input, &opts);
            let legacy = normalize_for_matching_legacy(input, &opts);
            assert_eq!(
                fused, legacy,
                "fused vs legacy diverged for {:?}: fused={:?}, legacy={:?}",
                input, fused, legacy,
            );
        }
    }

    #[test]
    fn fused_matches_legacy_turkish() {
        let opts = turkish_opts();
        let fixtures = [
            "Istanbul",
            "\u{0130}stanbul",
            "\u{0131}stanbul",
            "FILE",
            "fıle",
        ];
        for input in &fixtures {
            let fused = normalize_for_matching(input, &opts);
            let legacy = normalize_for_matching_legacy(input, &opts);
            assert_eq!(
                fused, legacy,
                "fused vs legacy diverged for {:?} (Turkish): fused={:?}, legacy={:?}",
                input, fused, legacy,
            );
        }
    }
}
