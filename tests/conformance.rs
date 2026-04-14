// tests/conformance.rs
//! Unicode NormalizationTest.txt conformance suite.
//!
//! Validates all ~20,000 test cases from the official UAX #15 test data
//! against `simd_normalizer`'s NFC, NFD, NFKC, and NFKD implementations.

mod data;
use data::normalization_tests::NORMALIZATION_TESTS;
use simd_normalizer::UnicodeNormalization;

#[test]
fn uax15_nfc_invariants() {
    for (i, t) in NORMALIZATION_TESTS.iter().enumerate() {
        let r1 = t.source.nfc();
        let r2 = t.nfc.nfc();
        let r3 = t.nfd.nfc();
        assert_eq!(
            &*r1, t.nfc,
            "NFC(source) != nfc at test case {i}: source={:?}",
            t.source
        );
        assert_eq!(&*r2, t.nfc, "NFC(nfc) != nfc at test case {i}");
        assert_eq!(&*r3, t.nfc, "NFC(nfd) != nfc at test case {i}");

        let r4 = t.nfkc.nfc();
        let r5 = t.nfkd.nfc();
        assert_eq!(&*r4, t.nfkc, "NFC(nfkc) != nfkc at test case {i}");
        assert_eq!(&*r5, t.nfkc, "NFC(nfkd) != nfkc at test case {i}");
    }
}

#[test]
fn uax15_nfd_invariants() {
    for (i, t) in NORMALIZATION_TESTS.iter().enumerate() {
        let r1 = t.source.nfd();
        let r2 = t.nfc.nfd();
        let r3 = t.nfd.nfd();
        assert_eq!(
            &*r1, t.nfd,
            "NFD(source) != nfd at test case {i}: source={:?}",
            t.source
        );
        assert_eq!(&*r2, t.nfd, "NFD(nfc) != nfd at test case {i}");
        assert_eq!(&*r3, t.nfd, "NFD(nfd) != nfd at test case {i}");

        let r4 = t.nfkc.nfd();
        let r5 = t.nfkd.nfd();
        assert_eq!(&*r4, t.nfkd, "NFD(nfkc) != nfkd at test case {i}");
        assert_eq!(&*r5, t.nfkd, "NFD(nfkd) != nfkd at test case {i}");
    }
}

#[test]
fn uax15_nfkc_invariants() {
    for (i, t) in NORMALIZATION_TESTS.iter().enumerate() {
        let r1 = t.source.nfkc();
        let r2 = t.nfc.nfkc();
        let r3 = t.nfd.nfkc();
        let r4 = t.nfkc.nfkc();
        let r5 = t.nfkd.nfkc();
        assert_eq!(
            &*r1, t.nfkc,
            "NFKC(source) != nfkc at test case {i}: source={:?}",
            t.source
        );
        assert_eq!(&*r2, t.nfkc, "NFKC(nfc) != nfkc at test case {i}");
        assert_eq!(&*r3, t.nfkc, "NFKC(nfd) != nfkc at test case {i}");
        assert_eq!(&*r4, t.nfkc, "NFKC(nfkc) != nfkc at test case {i}");
        assert_eq!(&*r5, t.nfkc, "NFKC(nfkd) != nfkc at test case {i}");
    }
}

#[test]
fn uax15_nfkd_invariants() {
    for (i, t) in NORMALIZATION_TESTS.iter().enumerate() {
        let r1 = t.source.nfkd();
        let r2 = t.nfc.nfkd();
        let r3 = t.nfd.nfkd();
        let r4 = t.nfkc.nfkd();
        let r5 = t.nfkd.nfkd();
        assert_eq!(
            &*r1, t.nfkd,
            "NFKD(source) != nfkd at test case {i}: source={:?}",
            t.source
        );
        assert_eq!(&*r2, t.nfkd, "NFKD(nfc) != nfkd at test case {i}");
        assert_eq!(&*r3, t.nfkd, "NFKD(nfd) != nfkd at test case {i}");
        assert_eq!(&*r4, t.nfkd, "NFKD(nfkc) != nfkd at test case {i}");
        assert_eq!(&*r5, t.nfkd, "NFKD(nfkd) != nfkd at test case {i}");
    }
}

#[test]
fn uax15_quick_check_on_normalized_forms() {
    for (i, t) in NORMALIZATION_TESTS.iter().enumerate() {
        assert!(
            t.nfc.is_nfc(),
            "is_nfc() false for nfc field at case {i}: {:?}",
            t.nfc
        );
        assert!(
            t.nfd.is_nfd(),
            "is_nfd() false for nfd field at case {i}: {:?}",
            t.nfd
        );
        assert!(
            t.nfkc.is_nfkc(),
            "is_nfkc() false for nfkc field at case {i}: {:?}",
            t.nfkc
        );
        assert!(
            t.nfkd.is_nfkd(),
            "is_nfkd() false for nfkd field at case {i}: {:?}",
            t.nfkd
        );
    }
}
