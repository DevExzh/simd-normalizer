#![no_main]

use libfuzzer_sys::fuzz_target;
use simd_normalizer::UnicodeNormalization;

fuzz_target!(|data: &str| {
    let n = simd_normalizer::nfd();
    let normalized = n.normalize(data);
    assert!(
        n.is_normalized(&normalized),
        "NFD normalize output not is_normalized"
    );
    let re_normalized = n.normalize(&normalized);
    assert_eq!(normalized, re_normalized, "NFD not idempotent");
    let _ = n.quick_check(data);
    let _ = data.nfd();
    let _ = data.is_nfd();
});
