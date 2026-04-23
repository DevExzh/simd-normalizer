//! Differential proptest: fast-path `quick_check_*` vs slow-path
//! `quick_check_*_oracle`. Guards against regressions in the safe-lead
//! byte classifier (spec §3 Layer 2).
//!
//! Requires --features quick_check_oracle.

#![cfg(feature = "quick_check_oracle")]

use proptest::prelude::*;
use simd_normalizer::{
    quick_check_nfc, quick_check_nfc_oracle, quick_check_nfd, quick_check_nfd_oracle,
    quick_check_nfkc, quick_check_nfkc_oracle, quick_check_nfkd, quick_check_nfkd_oracle,
};

// ---------------------------------------------------------------------------
// Strategy 1 — general Unicode mix.
// ---------------------------------------------------------------------------

fn unicode_string_strategy() -> impl Strategy<Value = String> {
    let ranges = prop::char::ranges(std::borrow::Cow::Borrowed(&[
        '\u{0020}'..='\u{007E}',
        '\u{0100}'..='\u{024F}',
        '\u{0300}'..='\u{036F}',
        '\u{0400}'..='\u{04FF}',
        '\u{0600}'..='\u{06FF}',
        '\u{0900}'..='\u{097F}',
        '\u{1100}'..='\u{11FF}',
        '\u{3040}'..='\u{309F}',
        '\u{4E00}'..='\u{4FFF}',
        '\u{AC00}'..='\u{D7A3}',
        '\u{1F600}'..='\u{1F64F}',
    ]));
    prop::collection::vec(ranges, 1..64).prop_map(|cs| cs.into_iter().collect::<String>())
}

// ---------------------------------------------------------------------------
// Strategy 2 — adversarial: 60..80 bytes, safe-byte CJK near pos 63
// (the 64-byte SIMD chunk boundary), interleaved with combining marks.
// ---------------------------------------------------------------------------

fn adversarial_strategy() -> impl Strategy<Value = String> {
    let tokens = prop::collection::vec(
        prop_oneof![
            // safe-lead CJK, 3 bytes (NFC/NFD/NFKC/NFKD-safe)
            (0x4E00u32..=0x9FFFu32).prop_map(|cp| char::from_u32(cp).unwrap().to_string()),
            // safe-lead Hangul syllable, 3 bytes (NFC/NFKC safe; NFD/NFKD takes decode path)
            (0xB000u32..=0xCFFFu32).prop_map(|cp| char::from_u32(cp).unwrap().to_string()),
            // combining mark, 2 bytes, CCC>0
            (0x0300u32..=0x036Fu32).prop_map(|cp| char::from_u32(cp).unwrap().to_string()),
            // ASCII padding, 1 byte
            Just("x".to_string()),
        ],
        8..40,
    );
    tokens.prop_map(|v| {
        let mut s: String = v.concat();
        while s.len() > 80 {
            s.pop(); // pop() removes a full char so we stay UTF-8-valid.
        }
        while s.len() < 60 {
            s.push('x');
        }
        s
    })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 8192, .. ProptestConfig::default() })]

    #[test]
    fn nfc_fastpath_matches_oracle_general(s in unicode_string_strategy()) {
        prop_assert_eq!(quick_check_nfc(&s), quick_check_nfc_oracle(&s), "input: {:?}", s);
    }

    #[test]
    fn nfd_fastpath_matches_oracle_general(s in unicode_string_strategy()) {
        prop_assert_eq!(quick_check_nfd(&s), quick_check_nfd_oracle(&s), "input: {:?}", s);
    }

    #[test]
    fn nfkc_fastpath_matches_oracle_general(s in unicode_string_strategy()) {
        prop_assert_eq!(quick_check_nfkc(&s), quick_check_nfkc_oracle(&s), "input: {:?}", s);
    }

    #[test]
    fn nfkd_fastpath_matches_oracle_general(s in unicode_string_strategy()) {
        prop_assert_eq!(quick_check_nfkd(&s), quick_check_nfkd_oracle(&s), "input: {:?}", s);
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 8192, .. ProptestConfig::default() })]

    #[test]
    fn nfc_fastpath_matches_oracle_adversarial(s in adversarial_strategy()) {
        prop_assert_eq!(quick_check_nfc(&s), quick_check_nfc_oracle(&s), "input: {:?}", s);
    }

    #[test]
    fn nfd_fastpath_matches_oracle_adversarial(s in adversarial_strategy()) {
        prop_assert_eq!(quick_check_nfd(&s), quick_check_nfd_oracle(&s), "input: {:?}", s);
    }

    #[test]
    fn nfkc_fastpath_matches_oracle_adversarial(s in adversarial_strategy()) {
        prop_assert_eq!(quick_check_nfkc(&s), quick_check_nfkc_oracle(&s), "input: {:?}", s);
    }

    #[test]
    fn nfkd_fastpath_matches_oracle_adversarial(s in adversarial_strategy()) {
        prop_assert_eq!(quick_check_nfkd(&s), quick_check_nfkd_oracle(&s), "input: {:?}", s);
    }
}
