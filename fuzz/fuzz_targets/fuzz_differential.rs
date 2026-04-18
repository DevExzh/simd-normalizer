#![no_main]

use libfuzzer_sys::fuzz_target;
use unicode_normalization::UnicodeNormalization as RefNormalization;

fuzz_target!(|data: &str| {
    let ours_nfc = simd_normalizer::nfc().normalize(data).into_owned();
    let theirs_nfc: String = data.nfc().collect();
    assert_eq!(
        ours_nfc, theirs_nfc,
        "NFC: differential mismatch on input {:?}",
        data
    );

    let ours_nfd = simd_normalizer::nfd().normalize(data).into_owned();
    let theirs_nfd: String = data.nfd().collect();
    assert_eq!(
        ours_nfd, theirs_nfd,
        "NFD: differential mismatch on input {:?}",
        data
    );

    let ours_nfkc = simd_normalizer::nfkc().normalize(data).into_owned();
    let theirs_nfkc: String = data.nfkc().collect();
    assert_eq!(
        ours_nfkc, theirs_nfkc,
        "NFKC: differential mismatch on input {:?}",
        data
    );

    let ours_nfkd = simd_normalizer::nfkd().normalize(data).into_owned();
    let theirs_nfkd: String = data.nfkd().collect();
    assert_eq!(
        ours_nfkd, theirs_nfkd,
        "NFKD: differential mismatch on input {:?}",
        data
    );
});
