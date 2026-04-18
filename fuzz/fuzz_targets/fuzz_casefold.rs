#![no_main]

use libfuzzer_sys::fuzz_target;
use simd_normalizer::{CaseFoldMode, casefold, casefold_char};

fuzz_target!(|data: &str| {
    for (mode_name, mode) in [
        ("Standard", CaseFoldMode::Standard),
        ("Turkish", CaseFoldMode::Turkish),
    ] {
        let folded = casefold(data, mode);
        let twice = casefold(&folded, mode);
        assert_eq!(
            twice.as_ref(),
            folded.as_ref(),
            "casefold not idempotent under {mode_name}"
        );

        for c in data.chars() {
            let _ = casefold_char(c, mode);
        }
    }

    if let Some(c) = data.chars().next() {
        let _ = casefold_char(c, CaseFoldMode::Standard);
        let _ = casefold_char(c, CaseFoldMode::Turkish);
    }
});
