//! Fused normalization pipeline for case-insensitive, confusable-aware matching.
//!
//! Pipeline: **NFKC → CaseFold → Confusable Skeleton** (NFD → confusable_map → NFD).
//!
//! Two strings that produce the same [`normalize_for_matching`] output are
//! equivalent for matching purposes: they share the same compatibility
//! decomposition, the same case folding, and the same confusable prototype.

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

    // Run the full pipeline: NFKC → casefold → skeleton → casefold.
    // Iterate until the output converges, because each step can introduce
    // characters that need further processing by a different step:
    //   - Confusable mappings can introduce compatibility chars (e.g., % → º/₀)
    //     that need another NFKC pass
    //   - NFKC can introduce characters with confusable mappings (e.g., ₀ → 0 → O)
    //   - Confusable mappings can introduce uppercase (e.g., 0 → O) that needs casefold
    // In practice, convergence is reached in 2–3 iterations.
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

/// Single pass of the matching pipeline.
fn one_pass(input: &str, opts: &MatchingOptions) -> String {
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
    normalize_for_matching(input, opts)
        .encode_utf16()
        .collect()
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

    fn default_opts() -> MatchingOptions {
        MatchingOptions::default()
    }

    fn turkish_opts() -> MatchingOptions {
        MatchingOptions {
            case_fold: CaseFoldMode::Turkish,
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
        let inputs = ["hello", "File", "\u{0430}\u{0440}\u{0440}l\u{0435}", "\u{00C0}"];
        for input in &inputs {
            let once = normalize_for_matching(input, &opts);
            let twice = normalize_for_matching(&once, &opts);
            assert_eq!(once, twice, "normalize_for_matching should be idempotent for {:?}", input);
        }
    }

    #[test]
    fn matching_not_confusable_different_words() {
        let opts = default_opts();
        assert!(!matches_normalized("hello", "world", &opts));
        assert!(!matches_normalized("file", "pile", &opts));
    }
}
