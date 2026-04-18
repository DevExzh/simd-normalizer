#![no_main]

use libfuzzer_sys::fuzz_target;
use simd_normalizer::{are_confusable, skeleton};

fuzz_target!(|data: &str| {
    let sk = skeleton(data);

    assert!(
        are_confusable(data, data),
        "are_confusable reflexivity failed"
    );

    let sk_twice = skeleton(&sk);
    assert_eq!(sk_twice, sk, "skeleton not idempotent");

    // Symmetry: split at a valid char boundary; skip if input is empty.
    if !data.is_empty() {
        let mid_char = data.chars().count() / 2;
        let mid = data
            .char_indices()
            .nth(mid_char)
            .map(|(i, _)| i)
            .unwrap_or(data.len());
        let (a, b) = data.split_at(mid);
        assert_eq!(
            are_confusable(a, b),
            are_confusable(b, a),
            "are_confusable symmetry failed"
        );
    }
});
