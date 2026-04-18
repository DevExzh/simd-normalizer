#![no_main]

use libfuzzer_sys::fuzz_target;
use simd_normalizer::UnicodeNormalization;

fuzz_target!(|data: &str| {
    let n = simd_normalizer::nfkd();
    let normalized = n.normalize(data);
    assert!(
        n.is_normalized(&normalized),
        "NFKD normalize output not is_normalized"
    );
    let re_normalized = n.normalize(&normalized);
    assert_eq!(normalized, re_normalized, "NFKD not idempotent");
    let _ = n.quick_check(data);
    let _ = data.nfkd();
    let _ = data.is_nfkd();
});
