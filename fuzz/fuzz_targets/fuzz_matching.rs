#![no_main]

use libfuzzer_sys::fuzz_target;
use simd_normalizer::CaseFoldMode;
use simd_normalizer::matching::{
    MatchingOptions, matches_normalized, normalize_for_matching, normalize_for_matching_utf16,
};

fuzz_target!(|data: &str| {
    for (mode_name, mode) in [
        ("Standard", CaseFoldMode::Standard),
        ("Turkish", CaseFoldMode::Turkish),
    ] {
        let opts = MatchingOptions {
            case_fold: mode,
            ..MatchingOptions::default()
        };

        let normalized = normalize_for_matching(data, &opts);
        let utf16 = normalize_for_matching_utf16(data, &opts);

        assert!(
            matches_normalized(data, data, &opts),
            "matches_normalized reflexivity failed under {mode_name}"
        );

        let decoded = String::from_utf16(&utf16)
            .unwrap_or_else(|e| panic!("UTF-16 output malformed under {mode_name}: {e}"));
        assert_eq!(
            decoded, normalized,
            "UTF-16 round-trip mismatch under {mode_name}"
        );
    }
});
