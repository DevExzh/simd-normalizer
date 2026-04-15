//! Property-based tests for normalization invariants.
//!
//! Uses proptest to verify algebraic properties: idempotence, round-trip stability,
//! Cow::Borrowed correctness, combining-heavy sequences, Hangul coverage, and
//! is_normalized consistency.

use proptest::prelude::*;
use simd_normalizer::UnicodeNormalization;
use simd_normalizer::{CaseFoldMode, casefold, skeleton};
use simd_normalizer::matching::{MatchingOptions, normalize_for_matching};
use std::borrow::Cow;

// ---------------------------------------------------------------------------
// Strategy generators
// ---------------------------------------------------------------------------

/// Broad mix of Unicode scripts for general property testing.
fn unicode_string_strategy() -> impl Strategy<Value = String> {
    let ranges = prop::char::ranges(std::borrow::Cow::Borrowed(&[
        // ASCII
        '\u{0020}'..='\u{007E}',
        // Latin Extended-A / Extended-B
        '\u{0100}'..='\u{024F}',
        // Combining Diacritical Marks
        '\u{0300}'..='\u{036F}',
        // Cyrillic
        '\u{0400}'..='\u{04FF}',
        // Arabic
        '\u{0600}'..='\u{06FF}',
        // Devanagari
        '\u{0900}'..='\u{097F}',
        // Hangul Jamo
        '\u{1100}'..='\u{11FF}',
        // Hiragana
        '\u{3040}'..='\u{309F}',
        // CJK Unified Ideographs (small slice)
        '\u{4E00}'..='\u{4FFF}',
        // Hangul Syllables (small slice)
        '\u{AC00}'..='\u{D7A3}',
        // Emoticons
        '\u{1F600}'..='\u{1F64F}',
    ]));
    prop::collection::vec(ranges, 1..64).prop_map(|chars| chars.into_iter().collect::<String>())
}

/// Strings with a base character followed by 1-8 combining marks.
fn combining_heavy_strategy() -> impl Strategy<Value = String> {
    let base_chars = prop::char::ranges(std::borrow::Cow::Borrowed(&[
        '\u{0041}'..='\u{005A}', // A-Z
        '\u{0061}'..='\u{007A}', // a-z
        '\u{00C0}'..='\u{00FF}', // Latin-1 Supplement
    ]));
    let combining = prop::char::ranges(std::borrow::Cow::Borrowed(&[
        '\u{0300}'..='\u{036F}', // Combining Diacritical Marks
    ]));

    prop::collection::vec((base_chars, prop::collection::vec(combining, 1..=8)), 1..=8).prop_map(
        |segments| {
            let mut s = String::new();
            for (base, marks) in segments {
                s.push(base);
                for m in marks {
                    s.push(m);
                }
            }
            s
        },
    )
}

/// Hangul syllables and L+V / L+V+T jamo sequences.
fn hangul_strategy() -> impl Strategy<Value = String> {
    let syllables = prop::char::ranges(std::borrow::Cow::Borrowed(&[
        '\u{AC00}'..='\u{D7A3}', // Precomposed Hangul syllables
    ]));
    let leading = prop::char::ranges(std::borrow::Cow::Borrowed(&[
        '\u{1100}'..='\u{1112}', // Hangul Jamo Leading consonants (L)
    ]));
    let vowel = prop::char::ranges(std::borrow::Cow::Borrowed(&[
        '\u{1161}'..='\u{1175}', // Hangul Jamo Vowels (V)
    ]));
    let trailing = prop::char::ranges(std::borrow::Cow::Borrowed(&[
        '\u{11A8}'..='\u{11C2}', // Hangul Jamo Trailing consonants (T)
    ]));

    // Mix of: precomposed syllables, L+V pairs, L+V+T triples
    prop::collection::vec(
        prop_oneof![
            syllables.prop_map(|c| vec![c]),
            (leading.clone(), vowel.clone()).prop_map(|(l, v)| vec![l, v]),
            (leading, vowel, trailing).prop_map(|(l, v, t)| vec![l, v, t]),
        ],
        1..=16,
    )
    .prop_map(|groups| {
        let mut s = String::new();
        for group in groups {
            for ch in group {
                s.push(ch);
            }
        }
        s
    })
}

// ---------------------------------------------------------------------------
// Main property tests (2000 cases)
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    // --- Idempotence ---

    #[test]
    fn idempotence_nfc(s in unicode_string_strategy()) {
        let once = s.nfc();
        let twice = once.nfc();
        prop_assert_eq!(&*once, &*twice, "NFC is not idempotent");
    }

    #[test]
    fn idempotence_nfd(s in unicode_string_strategy()) {
        let once = s.nfd();
        let twice = once.nfd();
        prop_assert_eq!(&*once, &*twice, "NFD is not idempotent");
    }

    #[test]
    fn idempotence_nfkc(s in unicode_string_strategy()) {
        let once = s.nfkc();
        let twice = once.nfkc();
        prop_assert_eq!(&*once, &*twice, "NFKC is not idempotent");
    }

    #[test]
    fn idempotence_nfkd(s in unicode_string_strategy()) {
        let once = s.nfkd();
        let twice = once.nfkd();
        prop_assert_eq!(&*once, &*twice, "NFKD is not idempotent");
    }

    // --- Round-trip stability ---

    #[test]
    fn roundtrip_nfc_nfd(s in unicode_string_strategy()) {
        let nfc_s = s.nfc();
        let nfd_of_nfc = nfc_s.nfd();
        let via_nfd = nfd_of_nfc.nfc();
        prop_assert_eq!(&*nfc_s, &*via_nfd, "nfc(nfd(nfc(s))) != nfc(s)");
    }

    #[test]
    fn roundtrip_nfd_nfc(s in unicode_string_strategy()) {
        let nfd_s = s.nfd();
        let nfc_of_nfd = nfd_s.nfc();
        let via_nfc = nfc_of_nfd.nfd();
        prop_assert_eq!(&*nfd_s, &*via_nfc, "nfd(nfc(nfd(s))) != nfd(s)");
    }

    #[test]
    fn roundtrip_nfkc_nfkd(s in unicode_string_strategy()) {
        let nfkc_s = s.nfkc();
        let nfkd_of_nfkc = nfkc_s.nfkd();
        let via_nfkd = nfkd_of_nfkc.nfkc();
        prop_assert_eq!(&*nfkc_s, &*via_nfkd, "nfkc(nfkd(nfkc(s))) != nfkc(s)");
    }

    #[test]
    fn roundtrip_nfkd_nfkc(s in unicode_string_strategy()) {
        let nfkd_s = s.nfkd();
        let nfkc_of_nfkd = nfkd_s.nfkc();
        let via_nfkc = nfkc_of_nfkd.nfkd();
        prop_assert_eq!(&*nfkd_s, &*via_nfkc, "nfkd(nfkc(nfkd(s))) != nfkd(s)");
    }

    // --- Cow::Borrowed correctness ---

    #[test]
    fn cow_borrowed_nfc(s in unicode_string_strategy()) {
        let nfc_s = s.nfc();
        // After one normalization, the result must be NFC-normalized.
        // A second call should detect it's already normalized and return Borrowed.
        let nfc_str: &str = &nfc_s;
        let owned = nfc_str.to_string();
        let second = owned.as_str().nfc();
        if owned.as_str().is_nfc() {
            match &second {
                Cow::Borrowed(b) => {
                    prop_assert!(
                        core::ptr::eq(*b, owned.as_str()),
                        "NFC Cow::Borrowed pointer mismatch for already-NFC string"
                    );
                }
                Cow::Owned(_) => {
                    // is_nfc() said true, so nfc() should return Borrowed
                    prop_assert!(false, "is_nfc() returned true but nfc() returned Owned");
                }
            }
        }
    }

    #[test]
    fn cow_borrowed_nfd(s in unicode_string_strategy()) {
        let nfd_s = s.nfd();
        let nfd_str: &str = &nfd_s;
        let owned = nfd_str.to_string();
        let second = owned.as_str().nfd();
        if owned.as_str().is_nfd() {
            match &second {
                Cow::Borrowed(b) => {
                    prop_assert!(
                        core::ptr::eq(*b, owned.as_str()),
                        "NFD Cow::Borrowed pointer mismatch for already-NFD string"
                    );
                }
                Cow::Owned(_) => {
                    prop_assert!(false, "is_nfd() returned true but nfd() returned Owned");
                }
            }
        }
    }

    #[test]
    fn cow_borrowed_nfkc(s in unicode_string_strategy()) {
        let nfkc_s = s.nfkc();
        let nfkc_str: &str = &nfkc_s;
        let owned = nfkc_str.to_string();
        let second = owned.as_str().nfkc();
        if owned.as_str().is_nfkc() {
            match &second {
                Cow::Borrowed(b) => {
                    prop_assert!(
                        core::ptr::eq(*b, owned.as_str()),
                        "NFKC Cow::Borrowed pointer mismatch for already-NFKC string"
                    );
                }
                Cow::Owned(_) => {
                    prop_assert!(false, "is_nfkc() returned true but nfkc() returned Owned");
                }
            }
        }
    }

    #[test]
    fn cow_borrowed_nfkd(s in unicode_string_strategy()) {
        let nfkd_s = s.nfkd();
        let nfkd_str: &str = &nfkd_s;
        let owned = nfkd_str.to_string();
        let second = owned.as_str().nfkd();
        if owned.as_str().is_nfkd() {
            match &second {
                Cow::Borrowed(b) => {
                    prop_assert!(
                        core::ptr::eq(*b, owned.as_str()),
                        "NFKD Cow::Borrowed pointer mismatch for already-NFKD string"
                    );
                }
                Cow::Owned(_) => {
                    prop_assert!(false, "is_nfkd() returned true but nfkd() returned Owned");
                }
            }
        }
    }

    // --- is_normalized consistency ---

    #[test]
    fn is_nfc_consistency(s in unicode_string_strategy()) {
        let nfc_s = s.nfc();
        if &*nfc_s == s.as_str() {
            prop_assert!(s.is_nfc(), "nfc(s) == s but is_nfc(s) is false");
        }
    }

    #[test]
    fn is_nfd_consistency(s in unicode_string_strategy()) {
        let nfd_s = s.nfd();
        if &*nfd_s == s.as_str() {
            prop_assert!(s.is_nfd(), "nfd(s) == s but is_nfd(s) is false");
        }
    }

    #[test]
    fn is_nfkc_consistency(s in unicode_string_strategy()) {
        let nfkc_s = s.nfkc();
        if &*nfkc_s == s.as_str() {
            prop_assert!(s.is_nfkc(), "nfkc(s) == s but is_nfkc(s) is false");
        }
    }

    #[test]
    fn is_nfkd_consistency(s in unicode_string_strategy()) {
        let nfkd_s = s.nfkd();
        if &*nfkd_s == s.as_str() {
            prop_assert!(s.is_nfkd(), "nfkd(s) == s but is_nfkd(s) is false");
        }
    }
}

// ---------------------------------------------------------------------------
// Combining-heavy property tests (1000 cases)
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn combining_heavy_idempotence_nfc(s in combining_heavy_strategy()) {
        let once = s.nfc();
        let twice = once.nfc();
        prop_assert_eq!(&*once, &*twice, "NFC not idempotent on combining-heavy input");
    }

    #[test]
    fn combining_heavy_idempotence_nfd(s in combining_heavy_strategy()) {
        let once = s.nfd();
        let twice = once.nfd();
        prop_assert_eq!(&*once, &*twice, "NFD not idempotent on combining-heavy input");
    }

    #[test]
    fn combining_heavy_idempotence_nfkc(s in combining_heavy_strategy()) {
        let once = s.nfkc();
        let twice = once.nfkc();
        prop_assert_eq!(&*once, &*twice, "NFKC not idempotent on combining-heavy input");
    }

    #[test]
    fn combining_heavy_idempotence_nfkd(s in combining_heavy_strategy()) {
        let once = s.nfkd();
        let twice = once.nfkd();
        prop_assert_eq!(&*once, &*twice, "NFKD not idempotent on combining-heavy input");
    }

    #[test]
    fn combining_heavy_roundtrip_nfc_nfd(s in combining_heavy_strategy()) {
        let nfc_s = s.nfc();
        let nfd_of_nfc = nfc_s.nfd();
        let via_nfd = nfd_of_nfc.nfc();
        prop_assert_eq!(&*nfc_s, &*via_nfd, "NFC->NFD->NFC round-trip failed on combining-heavy");
    }

    #[test]
    fn combining_heavy_roundtrip_nfd_nfc(s in combining_heavy_strategy()) {
        let nfd_s = s.nfd();
        let nfc_of_nfd = nfd_s.nfc();
        let via_nfc = nfc_of_nfd.nfd();
        prop_assert_eq!(&*nfd_s, &*via_nfc, "NFD->NFC->NFD round-trip failed on combining-heavy");
    }
}

// ---------------------------------------------------------------------------
// Hangul property tests (1000 cases)
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn hangul_idempotence_nfc(s in hangul_strategy()) {
        let once = s.nfc();
        let twice = once.nfc();
        prop_assert_eq!(&*once, &*twice, "NFC not idempotent on Hangul input");
    }

    #[test]
    fn hangul_idempotence_nfd(s in hangul_strategy()) {
        let once = s.nfd();
        let twice = once.nfd();
        prop_assert_eq!(&*once, &*twice, "NFD not idempotent on Hangul input");
    }

    #[test]
    fn hangul_roundtrip_nfc_nfd(s in hangul_strategy()) {
        let nfc_s = s.nfc();
        let nfd_of_nfc = nfc_s.nfd();
        let via_nfd = nfd_of_nfc.nfc();
        prop_assert_eq!(&*nfc_s, &*via_nfd, "Hangul NFC->NFD->NFC round-trip failed");
    }

    #[test]
    fn hangul_roundtrip_nfd_nfc(s in hangul_strategy()) {
        let nfd_s = s.nfd();
        let nfc_of_nfd = nfd_s.nfc();
        let via_nfc = nfc_of_nfd.nfd();
        prop_assert_eq!(&*nfd_s, &*via_nfc, "Hangul NFD->NFC->NFD round-trip failed");
    }
}

// ---------------------------------------------------------------------------
// Case folding property tests (1000 cases)
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn casefold_idempotent(s in unicode_string_strategy()) {
        let once = casefold(&s, CaseFoldMode::Standard);
        let twice = casefold(&once, CaseFoldMode::Standard);
        prop_assert_eq!(&*once, &*twice, "casefold not idempotent");
    }

    #[test]
    fn casefold_turkish_idempotent(s in unicode_string_strategy()) {
        let once = casefold(&s, CaseFoldMode::Turkish);
        let twice = casefold(&once, CaseFoldMode::Turkish);
        prop_assert_eq!(&*once, &*twice, "Turkish casefold not idempotent");
    }
}

// ---------------------------------------------------------------------------
// Skeleton property tests (500 cases -- skeleton is more expensive)
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn skeleton_converges_in_two_passes(s in unicode_string_strategy()) {
        // UTS #39 skeleton is not guaranteed to be idempotent in one pass,
        // but applying it twice should reach a fixed point.
        let once = skeleton(&s);
        let twice = skeleton(&once);
        let thrice = skeleton(&twice);
        prop_assert_eq!(twice, thrice, "skeleton did not converge after two passes");
    }
}

// ---------------------------------------------------------------------------
// Matching pipeline property tests (500 cases)
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn matching_idempotent(s in unicode_string_strategy()) {
        let opts = MatchingOptions::default();
        let once = normalize_for_matching(&s, &opts);
        let twice = normalize_for_matching(&once, &opts);
        prop_assert_eq!(once, twice, "normalize_for_matching not idempotent");
    }
}

// ---------------------------------------------------------------------------
// Exhaustive Hangul syllable test (deterministic, not proptest)
// ---------------------------------------------------------------------------

#[test]
fn hangul_exhaustive_all_11172_syllables() {
    const S_BASE: u32 = 0xAC00;
    const S_COUNT: u32 = 11172;
    for offset in 0..S_COUNT {
        let cp = S_BASE + offset;
        let syllable = char::from_u32(cp).expect("valid Hangul syllable");
        let input = String::from(syllable);

        // Decompose to NFD: all resulting chars must be Hangul Jamo
        let decomposed = input.nfd();
        for ch in decomposed.chars() {
            assert!(
                (0x1100..=0x11FF).contains(&(ch as u32)),
                "Hangul U+{cp:04X} decomposed to non-Jamo U+{:04X}",
                ch as u32
            );
        }

        // Recompose: must round-trip back to the original syllable
        let recomposed = decomposed.nfc();
        assert_eq!(
            &*recomposed, &*input,
            "Hangul U+{cp:04X} didn't round-trip NFC(NFD(s))"
        );

        // The original precomposed syllable must be recognized as NFC
        assert!(input.is_nfc(), "Hangul U+{cp:04X} not recognized as NFC");
    }
}
