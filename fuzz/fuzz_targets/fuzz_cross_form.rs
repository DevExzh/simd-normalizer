#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    let nfc = simd_normalizer::nfc();
    let nfd = simd_normalizer::nfd();
    let nfkc = simd_normalizer::nfkc();
    let nfkd = simd_normalizer::nfkd();

    // Per-form consistency: normalize(x) must be is_normalized.
    assert!(
        nfc.is_normalized(&nfc.normalize(data)),
        "NFC: is_nfc(nfc(x)) must be true"
    );
    assert!(
        nfd.is_normalized(&nfd.normalize(data)),
        "NFD: is_nfd(nfd(x)) must be true"
    );
    assert!(
        nfkc.is_normalized(&nfkc.normalize(data)),
        "NFKC: is_nfkc(nfkc(x)) must be true"
    );
    assert!(
        nfkd.is_normalized(&nfkd.normalize(data)),
        "NFKD: is_nfkd(nfkd(x)) must be true"
    );

    // Cross-form compositional invariants.
    let nfd_x = nfd.normalize(data);
    assert_eq!(
        nfc.normalize(&nfd_x),
        nfc.normalize(data),
        "nfc(nfd(x)) must equal nfc(x)"
    );

    let nfkd_x = nfkd.normalize(data);
    assert_eq!(
        nfkc.normalize(&nfkd_x),
        nfkc.normalize(data),
        "nfkc(nfkd(x)) must equal nfkc(x)"
    );

    let nfc_x = nfc.normalize(data);
    assert_eq!(
        nfd.normalize(&nfc_x),
        nfd.normalize(data),
        "nfd(nfc(x)) must equal nfd(x)"
    );

    let nfkc_x = nfkc.normalize(data);
    assert_eq!(
        nfkd.normalize(&nfkc_x),
        nfkd.normalize(data),
        "nfkd(nfkc(x)) must equal nfkd(x)"
    );

    // Form strength invariants: compat subsumes canonical.
    assert_eq!(
        nfkd.normalize(data),
        nfkd.normalize(&nfd_x),
        "nfkd(x) must equal nfkd(nfd(x))"
    );
    assert_eq!(
        nfkc.normalize(data),
        nfkc.normalize(&nfc_x),
        "nfkc(x) must equal nfkc(nfc(x))"
    );
});
