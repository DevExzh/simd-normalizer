//! Equivalence tests: `UnicodeNormalization` trait methods on `&str`
//! produce identical output to the corresponding free functions and
//! normalizer methods.
//!
//! Closes the gap identified in
//! `docs/superpowers/specs/2026-04-17-full-edge-case-coverage-design.md`
//! section 5.

use proptest::prelude::*;
use simd_normalizer::{
    NfcNormalizer, NfdNormalizer, NfkcNormalizer, NfkdNormalizer, UnicodeNormalization, nfc, nfd,
    nfkc, nfkd,
};

/// A curated mix of strings exercising ASCII, Latin-1, combining marks,
/// Hangul, supplementary plane, NFKC expansion, and the empty string.
fn curated_samples() -> &'static [&'static str] {
    &[
        "",
        "hello",
        "HELLO",
        "café",                 // precomposed é
        "cafe\u{0301}",         // e + combining acute
        "A\u{030A}",            // A + combining ring → Å under NFC
        "\u{00C5}",             // precomposed Å
        "\u{FB01}ne",           // ﬁne → NFKC "fine"
        "\u{FF21}",             // fullwidth A → NFKC "A"
        "\u{00B2}+\u{00B3}=5",  // superscripts
        "\u{1F600}",            // supplementary emoji
        "a\u{1F600}b",
        "\u{AC00}",             // Hangul syllable 가
        "\u{1100}\u{1161}",     // Hangul L+V that composes to 가
        "O\u{0308}\u{0301}",    // multiple combining marks
        "string with mixed \u{00D6}\u{FB01}\u{1F600}",
    ]
}

// ---------------------------------------------------------------------------
// Trait == free-fn == normalizer-method for each form
// ---------------------------------------------------------------------------

#[test]
fn trait_nfc_matches_free_fn_and_normalizer_sample() {
    let normalizer = NfcNormalizer::new();
    for s in curated_samples() {
        let trait_out = s.nfc();
        let free_out = nfc().normalize(s);
        let normalizer_out = normalizer.normalize(s);
        assert_eq!(&*trait_out, &*free_out, "trait vs free-fn diverged on {s:?}");
        assert_eq!(
            &*free_out, &*normalizer_out,
            "free-fn vs normalizer diverged on {s:?}"
        );
    }
}

#[test]
fn trait_nfd_matches_free_fn_and_normalizer_sample() {
    let normalizer = NfdNormalizer::new();
    for s in curated_samples() {
        let trait_out = s.nfd();
        let free_out = nfd().normalize(s);
        let normalizer_out = normalizer.normalize(s);
        assert_eq!(&*trait_out, &*free_out, "trait vs free-fn diverged on {s:?}");
        assert_eq!(
            &*free_out, &*normalizer_out,
            "free-fn vs normalizer diverged on {s:?}"
        );
    }
}

#[test]
fn trait_nfkc_matches_free_fn_and_normalizer_sample() {
    let normalizer = NfkcNormalizer::new();
    for s in curated_samples() {
        let trait_out = s.nfkc();
        let free_out = nfkc().normalize(s);
        let normalizer_out = normalizer.normalize(s);
        assert_eq!(&*trait_out, &*free_out, "trait vs free-fn diverged on {s:?}");
        assert_eq!(
            &*free_out, &*normalizer_out,
            "free-fn vs normalizer diverged on {s:?}"
        );
    }
}

#[test]
fn trait_nfkd_matches_free_fn_and_normalizer_sample() {
    let normalizer = NfkdNormalizer::new();
    for s in curated_samples() {
        let trait_out = s.nfkd();
        let free_out = nfkd().normalize(s);
        let normalizer_out = normalizer.normalize(s);
        assert_eq!(&*trait_out, &*free_out, "trait vs free-fn diverged on {s:?}");
        assert_eq!(
            &*free_out, &*normalizer_out,
            "free-fn vs normalizer diverged on {s:?}"
        );
    }
}

#[test]
fn trait_is_normalized_matches_normalizer_sample() {
    for s in curated_samples() {
        assert_eq!(s.is_nfc(), NfcNormalizer::new().is_normalized(s), "is_nfc on {s:?}");
        assert_eq!(s.is_nfd(), NfdNormalizer::new().is_normalized(s), "is_nfd on {s:?}");
        assert_eq!(s.is_nfkc(), NfkcNormalizer::new().is_normalized(s), "is_nfkc on {s:?}");
        assert_eq!(s.is_nfkd(), NfkdNormalizer::new().is_normalized(s), "is_nfkd on {s:?}");
    }
}

// ---------------------------------------------------------------------------
// Property: trait == free-fn == normalizer, across ASCII + BMP + supplementary
// ---------------------------------------------------------------------------

fn mixed_string_strategy() -> impl Strategy<Value = String> {
    let ranges = prop::char::ranges(std::borrow::Cow::Borrowed(&[
        // ASCII printable
        '\u{0020}'..='\u{007E}',
        // Latin-1 Supplement (precomposed)
        '\u{00C0}'..='\u{00FF}',
        // Combining Diacritical Marks
        '\u{0300}'..='\u{036F}',
        // Hangul Syllables (small slice)
        '\u{AC00}'..='\u{AC10}',
        // Fullwidth (NFKC expansion)
        '\u{FF01}'..='\u{FF5E}',
        // Emoticons (supplementary)
        '\u{1F600}'..='\u{1F64F}',
    ]));
    prop::collection::vec(ranges, 0..32).prop_map(|chars| chars.into_iter().collect())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn trait_equals_free_fn_equals_normalizer(s in mixed_string_strategy()) {
        let nfc_normalizer = NfcNormalizer::new();
        let nfd_normalizer = NfdNormalizer::new();
        let nfkc_normalizer = NfkcNormalizer::new();
        let nfkd_normalizer = NfkdNormalizer::new();

        prop_assert_eq!(&*s.nfc(), &*nfc().normalize(&s));
        prop_assert_eq!(&*nfc().normalize(&s), &*nfc_normalizer.normalize(&s));

        prop_assert_eq!(&*s.nfd(), &*nfd().normalize(&s));
        prop_assert_eq!(&*nfd().normalize(&s), &*nfd_normalizer.normalize(&s));

        prop_assert_eq!(&*s.nfkc(), &*nfkc().normalize(&s));
        prop_assert_eq!(&*nfkc().normalize(&s), &*nfkc_normalizer.normalize(&s));

        prop_assert_eq!(&*s.nfkd(), &*nfkd().normalize(&s));
        prop_assert_eq!(&*nfkd().normalize(&s), &*nfkd_normalizer.normalize(&s));

        prop_assert_eq!(s.as_str().is_nfc(), nfc_normalizer.is_normalized(&s));
        prop_assert_eq!(s.as_str().is_nfd(), nfd_normalizer.is_normalized(&s));
        prop_assert_eq!(s.as_str().is_nfkc(), nfkc_normalizer.is_normalized(&s));
        prop_assert_eq!(s.as_str().is_nfkd(), nfkd_normalizer.is_normalized(&s));
    }
}
