#![no_main]

use libfuzzer_sys::fuzz_target;
use simd_normalizer::UnicodeNormalization;

fuzz_target!(|data: &str| {
    let n = simd_normalizer::nfc();
    let normalized = n.normalize(data);
    assert!(
        n.is_normalized(&normalized),
        "NFC normalize output not is_normalized"
    );
    let re_normalized = n.normalize(&normalized);
    assert_eq!(normalized, re_normalized, "NFC not idempotent");
    let _ = n.quick_check(data);
    let _ = data.nfc();
    let _ = data.is_nfc();
});
