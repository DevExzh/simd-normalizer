//! Parity regression tests for the fused matching pipeline.
//!
//! Every input produced by the proptest generator must yield the same output
//! from `normalize_for_matching` (fused) and `normalize_for_matching_legacy`
//! (the pre-fusion implementation).

use proptest::prelude::*;
use simd_normalizer::matching::{
    MatchingOptions, normalize_for_matching, normalize_for_matching_legacy,
};

fn default_opts() -> MatchingOptions {
    MatchingOptions::default()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    #[test]
    fn fused_equals_legacy_ascii(s in "\\PC{0,64}") {
        let opts = default_opts();
        prop_assert_eq!(
            normalize_for_matching(&s, &opts),
            normalize_for_matching_legacy(&s, &opts),
        );
    }

    #[test]
    fn fused_equals_legacy_cyrillic_latin(s in "[a-zA-Z\u{0400}-\u{04FF}]{0,32}") {
        let opts = default_opts();
        prop_assert_eq!(
            normalize_for_matching(&s, &opts),
            normalize_for_matching_legacy(&s, &opts),
        );
    }

    #[test]
    fn fused_equals_legacy_fullwidth(s in "[\u{FF00}-\u{FFEF}a-z ]{0,32}") {
        let opts = default_opts();
        prop_assert_eq!(
            normalize_for_matching(&s, &opts),
            normalize_for_matching_legacy(&s, &opts),
        );
    }
}

#[test]
fn fused_equals_legacy_specific_cases() {
    let opts = default_opts();
    for case in [
        "File",
        "FILE",
        "fıle",
        "apple",
        "\u{0430}\u{0440}\u{0440}le",
        "\u{FF21}",
        "Straße",
        "Istanbul",
        "",
        "a",
        "\u{1F600}hello",
        "e\u{0301}quipe",
        "\u{1F80}",
        "\u{0345}",
    ] {
        // minimized U+0345 divergence from the abandoned full-fusion attempt
        assert_eq!(
            normalize_for_matching(case, &opts),
            normalize_for_matching_legacy(case, &opts),
            "case mismatch: {:?}",
            case,
        );
    }
}
