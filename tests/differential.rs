//! Differential tests comparing simd-normalizer against the
//! `unicode-normalization` crate.

use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Reference crate helpers (import trait inside function to avoid collision)
// ---------------------------------------------------------------------------

fn ref_nfc(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    s.nfc().collect::<String>()
}

fn ref_nfd(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    s.nfd().collect::<String>()
}

fn ref_nfkc(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    s.nfkc().collect::<String>()
}

fn ref_nfkd(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    s.nfkd().collect::<String>()
}

// ---------------------------------------------------------------------------
// Our crate helpers (use constructor API to avoid trait import)
// ---------------------------------------------------------------------------

fn our_nfc(s: &str) -> String {
    simd_normalizer::nfc().normalize(s).into_owned()
}

fn our_nfd(s: &str) -> String {
    simd_normalizer::nfd().normalize(s).into_owned()
}

fn our_nfkc(s: &str) -> String {
    simd_normalizer::nfkc().normalize(s).into_owned()
}

fn our_nfkd(s: &str) -> String {
    simd_normalizer::nfkd().normalize(s).into_owned()
}

// ---------------------------------------------------------------------------
// Proptest strategies
// ---------------------------------------------------------------------------

/// Strategy: broad unicode mix covering many scripts and block ranges.
fn broad_unicode_strategy() -> impl Strategy<Value = String> {
    let ranges = vec![
        // ASCII
        '\u{0020}'..='\u{007E}',
        // Latin Extended-A
        '\u{0100}'..='\u{017F}',
        // Latin Extended-B
        '\u{0180}'..='\u{024F}',
        // Combining Diacritical Marks
        '\u{0300}'..='\u{036F}',
        // Greek and Coptic
        '\u{0370}'..='\u{03FF}',
        // Cyrillic
        '\u{0400}'..='\u{04FF}',
        // Hebrew
        '\u{0590}'..='\u{05FF}',
        // Arabic
        '\u{0600}'..='\u{06FF}',
        // Devanagari
        '\u{0900}'..='\u{097F}',
        // Thai
        '\u{0E00}'..='\u{0E7F}',
        // Hangul Jamo
        '\u{1100}'..='\u{11FF}',
        // Hiragana
        '\u{3040}'..='\u{309F}',
        // Katakana
        '\u{30A0}'..='\u{30FF}',
        // CJK Unified Ideographs (subset)
        '\u{4E00}'..='\u{4FFF}',
        // Hangul Syllables (subset)
        '\u{AC00}'..='\u{AD00}',
        // Emoticons
        '\u{1F600}'..='\u{1F64F}',
    ];

    prop::collection::vec(prop::char::ranges(ranges.into()), 1..=64)
        .prop_map(|chars| chars.into_iter().collect::<String>())
}

/// Strategy: compatibility decomposition targets (ligatures, fractions,
/// fullwidth forms, enclosed alphanumerics, CJK compatibility).
fn compat_decomp_strategy() -> impl Strategy<Value = String> {
    let ranges = vec![
        // Vulgar Fractions (in Number Forms)
        '\u{2150}'..='\u{215F}',
        // Number Forms (Roman numerals, etc.)
        '\u{2160}'..='\u{2188}',
        // Enclosed Alphanumerics
        '\u{2460}'..='\u{24FF}',
        // CJK Compatibility
        '\u{3300}'..='\u{33FF}',
        // CJK Compatibility Ideographs (subset)
        '\u{F900}'..='\u{F9FF}',
        // Latin ligatures (Alphabetic Presentation Forms)
        '\u{FB00}'..='\u{FB06}',
        // Fullwidth ASCII variants
        '\u{FF01}'..='\u{FF5E}',
        // Halfwidth Katakana
        '\u{FF65}'..='\u{FF9F}',
        // Superscripts and Subscripts
        '\u{2070}'..='\u{209F}',
    ];

    prop::collection::vec(prop::char::ranges(ranges.into()), 1..=32)
        .prop_map(|chars| chars.into_iter().collect::<String>())
}

/// Strategy: long combining sequences (base character followed by many
/// combining marks from diverse scripts).
fn long_combining_strategy() -> impl Strategy<Value = String> {
    let base_chars = vec!['A'..='Z', 'a'..='z'];

    let combining_marks = vec![
        // Combining Diacritical Marks
        '\u{0300}'..='\u{036F}',
        // Combining Diacritical Marks Extended
        '\u{1AB0}'..='\u{1AFF}',
        // Combining Cyrillic
        '\u{0483}'..='\u{0489}',
        // Hebrew combining marks (points and accents)
        '\u{0591}'..='\u{05BD}',
        // Arabic combining marks (Fathah, Dammah, Kasrah, etc.)
        '\u{064B}'..='\u{065F}',
        // Thai combining marks (above vowels, tone marks)
        '\u{0E31}'..='\u{0E3A}',
        // Combining Diacritical Marks for Symbols
        '\u{20D0}'..='\u{20FF}',
    ];

    // Generate a base char, then 4..=30 combining marks appended.
    let base = prop::char::ranges(base_chars.into());
    let marks = prop::collection::vec(prop::char::ranges(combining_marks.into()), 4..=30);

    (base, marks).prop_map(|(b, ms)| {
        let mut s = String::with_capacity(1 + ms.len() * 4);
        s.push(b);
        for m in ms {
            s.push(m);
        }
        s
    })
}

// ---------------------------------------------------------------------------
// Helper: assertion with detailed diagnostics
// ---------------------------------------------------------------------------

fn assert_eq_normalized(form: &str, input: &str, ours: &str, reference: &str) {
    assert_eq!(
        ours,
        reference,
        "\n{form} divergence!\
         \n  input (len={ilen}): {input_escaped}\
         \n  ours  (len={olen}): {ours_escaped}\
         \n  ref   (len={rlen}): {ref_escaped}\
         \n  input code points: {input_cps}\
         \n  ours  code points: {ours_cps}\
         \n  ref   code points: {ref_cps}",
        form = form,
        ilen = input.len(),
        input_escaped = input.escape_unicode(),
        olen = ours.len(),
        ours_escaped = ours.escape_unicode(),
        rlen = reference.len(),
        ref_escaped = reference.escape_unicode(),
        input_cps = codepoints(input),
        ours_cps = codepoints(ours),
        ref_cps = codepoints(reference),
    );
}

fn codepoints(s: &str) -> String {
    s.chars()
        .map(|c| format!("U+{:04X}", c as u32))
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------------------------------------------------------------------------
// Proptest: 5000 cases -- broad unicode
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5000))]

    #[test]
    fn differential_nfc(s in broad_unicode_strategy()) {
        let ours = our_nfc(&s);
        let reference = ref_nfc(&s);
        assert_eq_normalized("NFC", &s, &ours, &reference);
    }

    #[test]
    fn differential_nfd(s in broad_unicode_strategy()) {
        let ours = our_nfd(&s);
        let reference = ref_nfd(&s);
        assert_eq_normalized("NFD", &s, &ours, &reference);
    }

    #[test]
    fn differential_nfkc(s in broad_unicode_strategy()) {
        let ours = our_nfkc(&s);
        let reference = ref_nfkc(&s);
        assert_eq_normalized("NFKC", &s, &ours, &reference);
    }

    #[test]
    fn differential_nfkd(s in broad_unicode_strategy()) {
        let ours = our_nfkd(&s);
        let reference = ref_nfkd(&s);
        assert_eq_normalized("NFKD", &s, &ours, &reference);
    }
}

// ---------------------------------------------------------------------------
// Proptest: 2000 cases -- compatibility decomposition targets
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    #[test]
    fn differential_nfkc_compat(s in compat_decomp_strategy()) {
        let ours = our_nfkc(&s);
        let reference = ref_nfkc(&s);
        assert_eq_normalized("NFKC-compat", &s, &ours, &reference);
    }

    #[test]
    fn differential_nfkd_compat(s in compat_decomp_strategy()) {
        let ours = our_nfkd(&s);
        let reference = ref_nfkd(&s);
        assert_eq_normalized("NFKD-compat", &s, &ours, &reference);
    }
}

// ---------------------------------------------------------------------------
// Proptest: 1000 cases -- long combining sequences
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn differential_nfc_long_combining(s in long_combining_strategy()) {
        let ours = our_nfc(&s);
        let reference = ref_nfc(&s);
        assert_eq_normalized("NFC-long-combining", &s, &ours, &reference);
    }

    #[test]
    fn differential_nfd_long_combining(s in long_combining_strategy()) {
        let ours = our_nfd(&s);
        let reference = ref_nfd(&s);
        assert_eq_normalized("NFD-long-combining", &s, &ours, &reference);
    }
}

// ---------------------------------------------------------------------------
// Deterministic edge cases
// ---------------------------------------------------------------------------

#[test]
fn differential_edge_cases() {
    let cases: &[(&str, &str)] = &[
        // Empty string
        ("empty", ""),
        // Pure ASCII
        ("ascii", "Hello, World! 0123456789"),
        // Precomposed e-acute (U+00E9)
        ("precomposed-e-acute", "\u{00E9}"),
        // Decomposed e-acute (U+0065 U+0301)
        ("decomposed-e-acute", "\u{0065}\u{0301}"),
        // A + combining ring above (U+0041 U+030A) -> should compose to U+00C5
        ("a-ring", "\u{0041}\u{030A}"),
        // Hangul syllable GA (U+AC00) = L(U+1100) + V(U+1161)
        ("hangul-ga", "\u{AC00}"),
        // Hangul syllable GAG (U+AC01) = L(U+1100) + V(U+1161) + T(U+11A8)
        ("hangul-gag", "\u{AC01}"),
        // Jamo sequence L+V -> should compose to syllable
        ("jamo-lv", "\u{1100}\u{1161}"),
        // Jamo sequence L+V+T -> should compose to syllable
        ("jamo-lvt", "\u{1100}\u{1161}\u{11A8}"),
        // fi ligature (U+FB01) -> compatibility decomposition to "fi"
        ("fi-ligature", "\u{FB01}"),
        // Ohm sign (U+2126) -> canonical decomposition to Greek capital omega (U+03A9)
        ("ohm-sign", "\u{2126}"),
        // Angstrom sign (U+212B) -> canonical decomposition to A-ring (U+00C5)
        ("angstrom", "\u{212B}"),
        // Hiragana GA (U+304C) = KA(U+304B) + dakuten(U+3099)
        ("hiragana-ga-precomposed", "\u{304C}"),
        // Hiragana KA + combining dakuten
        ("hiragana-ga-decomposed", "\u{304B}\u{3099}"),
        // CCC reorder: cedilla (ccc=202) before acute (ccc=230)
        ("ccc-reorder-1", "\u{0065}\u{0327}\u{0301}"),
        // CCC reorder: acute (ccc=230) before cedilla (ccc=202) -- should reorder
        ("ccc-reorder-2", "\u{0065}\u{0301}\u{0327}"),
        // Long combining sequence
        (
            "long-combining",
            "A\u{0300}\u{0301}\u{0302}\u{0303}\u{0304}\u{0305}\u{0306}\u{0307}\u{0308}\u{0309}\u{030A}\u{030B}\u{030C}\u{030D}\u{030E}\u{030F}",
        ),
        // Mixed scripts
        (
            "mixed-scripts",
            "Hello\u{0301} \u{0410}\u{0308} \u{05D0}\u{05B0} \u{0627}\u{064E} \u{3042}\u{3099}",
        ),
        // Emoji with ZWJ
        (
            "emoji-zwj",
            "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}",
        ),
        // Fullwidth ASCII
        ("fullwidth", "\u{FF21}\u{FF22}\u{FF23}"),
        // Already-NFC text with no changes needed
        ("already-nfc", "Caf\u{00E9} na\u{00EF}ve"),
        // String of only combining marks (no base character)
        ("orphan-combiners", "\u{0300}\u{0301}\u{0302}"),
        // Single supplementary character (emoji)
        ("supplementary", "\u{1F600}"),
        // Repeated decomposable character
        ("repeated-decomposable", "\u{00C0}\u{00C0}\u{00C0}\u{00C0}"),
    ];

    for (label, input) in cases {
        // NFC
        let our_nfc_result = our_nfc(input);
        let ref_nfc_result = ref_nfc(input);
        assert_eq!(
            our_nfc_result,
            ref_nfc_result,
            "NFC edge case {label:?} failed\n  input codepoints: {input_cps}\n  ours:  {ours_cps}\n  ref:   {ref_cps}",
            label = label,
            input_cps = codepoints(input),
            ours_cps = codepoints(&our_nfc_result),
            ref_cps = codepoints(&ref_nfc_result),
        );

        // NFD
        let our_nfd_result = our_nfd(input);
        let ref_nfd_result = ref_nfd(input);
        assert_eq!(
            our_nfd_result,
            ref_nfd_result,
            "NFD edge case {label:?} failed\n  input codepoints: {input_cps}\n  ours:  {ours_cps}\n  ref:   {ref_cps}",
            label = label,
            input_cps = codepoints(input),
            ours_cps = codepoints(&our_nfd_result),
            ref_cps = codepoints(&ref_nfd_result),
        );

        // NFKC
        let our_nfkc_result = our_nfkc(input);
        let ref_nfkc_result = ref_nfkc(input);
        assert_eq!(
            our_nfkc_result,
            ref_nfkc_result,
            "NFKC edge case {label:?} failed\n  input codepoints: {input_cps}\n  ours:  {ours_cps}\n  ref:   {ref_cps}",
            label = label,
            input_cps = codepoints(input),
            ours_cps = codepoints(&our_nfkc_result),
            ref_cps = codepoints(&ref_nfkc_result),
        );

        // NFKD
        let our_nfkd_result = our_nfkd(input);
        let ref_nfkd_result = ref_nfkd(input);
        assert_eq!(
            our_nfkd_result,
            ref_nfkd_result,
            "NFKD edge case {label:?} failed\n  input codepoints: {input_cps}\n  ours:  {ours_cps}\n  ref:   {ref_cps}",
            label = label,
            input_cps = codepoints(input),
            ours_cps = codepoints(&our_nfkd_result),
            ref_cps = codepoints(&ref_nfkd_result),
        );
    }
}
