// tests/differential_icu4x.rs
//! Proptest-based differential tests comparing simd-normalizer against
//! `icu_normalizer` (ICU4X) for randomly generated multi-character strings.
//!
//! This fills the gap between:
//! - `exhaustive.rs` (single-codepoint differential against ICU4X)
//! - `multilingual.rs` (real-world text differential against ICU4X)
//! - `differential.rs` (random multi-char strings against `unicode-normalization`)
//!
//! The strategies here are intentionally different from `differential.rs` to
//! avoid duplication and focus on script-specific cluster patterns that stress
//! composition, decomposition, and CCC reordering in multi-character contexts.

use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Our crate helpers
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
// ICU4X reference helpers
// ---------------------------------------------------------------------------

fn icu_nfc(s: &str) -> String {
    use icu_normalizer::ComposingNormalizerBorrowed;
    ComposingNormalizerBorrowed::new_nfc()
        .normalize(s)
        .into_owned()
}

fn icu_nfd(s: &str) -> String {
    use icu_normalizer::DecomposingNormalizerBorrowed;
    DecomposingNormalizerBorrowed::new_nfd()
        .normalize(s)
        .into_owned()
}

fn icu_nfkc(s: &str) -> String {
    use icu_normalizer::ComposingNormalizerBorrowed;
    ComposingNormalizerBorrowed::new_nfkc()
        .normalize(s)
        .into_owned()
}

fn icu_nfkd(s: &str) -> String {
    use icu_normalizer::DecomposingNormalizerBorrowed;
    DecomposingNormalizerBorrowed::new_nfkd()
        .normalize(s)
        .into_owned()
}

// ---------------------------------------------------------------------------
// unicode-normalization reference helpers (for triple-check)
// ---------------------------------------------------------------------------

fn un_nfc(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    s.nfc().collect::<String>()
}

fn un_nfd(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    s.nfd().collect::<String>()
}

fn un_nfkc(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    s.nfkc().collect::<String>()
}

fn un_nfkd(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    s.nfkd().collect::<String>()
}

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

fn codepoints(s: &str) -> String {
    s.chars()
        .map(|c| format!("U+{:04X}", c as u32))
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------------------------------------------------------------------------
// Assertion helper: detailed diagnostics on divergence
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

/// Triple-reference assertion: simd_normalizer, icu_normalizer, AND
/// unicode-normalization must all agree.
fn assert_triple(
    form: &str,
    input: &str,
    our_fn: fn(&str) -> String,
    icu_fn: fn(&str) -> String,
    un_fn: fn(&str) -> String,
) {
    let ours = our_fn(input);
    let icu = icu_fn(input);
    let un = un_fn(input);

    assert_eq!(
        ours, icu,
        "\n{form} triple-check: simd != icu4x\
         \n  input code points: {input_cps}\
         \n  simd  code points: {ours_cps}\
         \n  icu4x code points: {icu_cps}\
         \n  unic  code points: {un_cps}",
        form = form,
        input_cps = codepoints(input),
        ours_cps = codepoints(&ours),
        icu_cps = codepoints(&icu),
        un_cps = codepoints(&un),
    );

    assert_eq!(
        ours, un,
        "\n{form} triple-check: simd != unicode-normalization\
         \n  input code points: {input_cps}\
         \n  simd  code points: {ours_cps}\
         \n  icu4x code points: {icu_cps}\
         \n  unic  code points: {un_cps}",
        form = form,
        input_cps = codepoints(input),
        ours_cps = codepoints(&ours),
        icu_cps = codepoints(&icu),
        un_cps = codepoints(&un),
    );

    assert_eq!(
        icu, un,
        "\n{form} triple-check: icu4x != unicode-normalization (reference disagreement!)\
         \n  input code points: {input_cps}\
         \n  icu4x code points: {icu_cps}\
         \n  unic  code points: {un_cps}",
        form = form,
        input_cps = codepoints(input),
        icu_cps = codepoints(&icu),
        un_cps = codepoints(&un),
    );
}

// ===========================================================================
// Proptest strategies
// ===========================================================================

/// Strategy 1: Hangul -- mix of Jamo L, V, T, precomposed syllables, and ASCII.
fn hangul_strategy() -> impl Strategy<Value = String> {
    let ranges = vec![
        // Hangul Jamo L (Leading consonants)
        '\u{1100}'..='\u{1112}',
        // Hangul Jamo V (Vowels)
        '\u{1161}'..='\u{1175}',
        // Hangul Jamo T (Trailing consonants)
        '\u{11A7}'..='\u{11C2}',
        // Precomposed Hangul Syllables
        '\u{AC00}'..='\u{D7A3}',
        // ASCII (interspersed)
        '\u{0020}'..='\u{007E}',
    ];

    prop::collection::vec(prop::char::ranges(ranges.into()), 1..=32)
        .prop_map(|chars| chars.into_iter().collect::<String>())
}

/// Strategy 2: Arabic/Hebrew combining -- base letters with combining marks.
fn arabic_hebrew_combining_strategy() -> impl Strategy<Value = String> {
    let ranges = vec![
        // Arabic base letters
        '\u{0621}'..='\u{064A}',
        // Arabic combining marks (Fathah, Dammah, Kasrah, Shadda, Sukun, etc.)
        '\u{064B}'..='\u{065F}',
        // Arabic superscript alef
        '\u{0670}'..='\u{0670}',
        // Hebrew letters
        '\u{05D0}'..='\u{05EA}',
        // Hebrew points and accents
        '\u{0591}'..='\u{05C7}',
    ];

    prop::collection::vec(prop::char::ranges(ranges.into()), 1..=32)
        .prop_map(|chars| chars.into_iter().collect::<String>())
}

/// Strategy 3: Indic (Devanagari) clusters -- consonants, virama, vowel signs,
/// nukta, anusvara, chandrabindu.
fn indic_cluster_strategy() -> impl Strategy<Value = String> {
    let ranges = vec![
        // Devanagari consonants
        '\u{0915}'..='\u{0939}',
        // Virama (halant)
        '\u{094D}'..='\u{094D}',
        // Devanagari vowel signs
        '\u{093E}'..='\u{094C}',
        // Nukta
        '\u{093C}'..='\u{093C}',
        // Anusvara
        '\u{0902}'..='\u{0902}',
        // Chandrabindu
        '\u{0901}'..='\u{0901}',
    ];

    prop::collection::vec(prop::char::ranges(ranges.into()), 1..=32)
        .prop_map(|chars| chars.into_iter().collect::<String>())
}

/// Strategy 4: CJK + musical symbols -- compatibility ideographs, musical
/// symbols with decompositions, and CJK unified ideographs.
fn cjk_musical_strategy() -> impl Strategy<Value = String> {
    let ranges = vec![
        // CJK Compatibility Ideographs
        '\u{F900}'..='\u{FAFF}',
        // Musical symbols with decompositions (U+1D15E-U+1D164)
        '\u{1D15E}'..='\u{1D164}',
        // Musical symbols with decompositions (U+1D1BB-U+1D1C0)
        '\u{1D1BB}'..='\u{1D1C0}',
        // CJK Unified Ideographs (subset)
        '\u{4E00}'..='\u{9FFF}',
    ];

    prop::collection::vec(prop::char::ranges(ranges.into()), 1..=16)
        .prop_map(|chars| chars.into_iter().collect::<String>())
}

/// Strategy 5: Boundary stress -- mostly ASCII with occasional non-ASCII
/// inserted at varying positions to stress SIMD chunk boundaries (~64 bytes).
fn boundary_stress_strategy() -> impl Strategy<Value = String> {
    // Build strings of length 60-200 that are mostly ASCII with occasional
    // multi-byte characters injected at various positions.
    let ascii_char = prop::char::range('\u{0020}', '\u{007E}');
    let non_ascii_chars = vec![
        // 2-byte: precomposed Latin
        '\u{00E9}'..='\u{00FF}',
        // 3-byte: Hangul syllables (small range)
        '\u{AC00}'..='\u{AC0F}',
        // 3-byte: CJK compatibility ideographs
        '\u{F900}'..='\u{F90F}',
        // 4-byte: musical symbols
        '\u{1D15E}'..='\u{1D164}',
        // Combining marks (to test mid-chunk mark handling)
        '\u{0300}'..='\u{030F}',
    ];
    let non_ascii = prop::char::ranges(non_ascii_chars.into());

    // Generate a vec of "segments": each segment is either a run of ASCII
    // or a single non-ASCII character.
    let segment = prop_oneof![
        // 80% chance: short ASCII run (1-20 chars)
        8 => prop::collection::vec(ascii_char, 1..=20)
            .prop_map(|cs| cs.into_iter().collect::<String>()),
        // 20% chance: a single non-ASCII character
        2 => non_ascii.prop_map(|c| c.to_string()),
    ];

    prop::collection::vec(segment, 3..=15)
        .prop_map(|segments| segments.concat())
        .prop_filter("string too short", |s| s.len() >= 60)
        .prop_filter("string too long", |s| s.len() <= 250)
}

// ===========================================================================
// Proptest: 2000 cases -- Hangul
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    #[test]
    fn icu4x_hangul_nfc(s in hangul_strategy()) {
        assert_eq_normalized("NFC-hangul", &s, &our_nfc(&s), &icu_nfc(&s));
    }

    #[test]
    fn icu4x_hangul_nfd(s in hangul_strategy()) {
        assert_eq_normalized("NFD-hangul", &s, &our_nfd(&s), &icu_nfd(&s));
    }

    #[test]
    fn icu4x_hangul_nfkc(s in hangul_strategy()) {
        assert_eq_normalized("NFKC-hangul", &s, &our_nfkc(&s), &icu_nfkc(&s));
    }

    #[test]
    fn icu4x_hangul_nfkd(s in hangul_strategy()) {
        assert_eq_normalized("NFKD-hangul", &s, &our_nfkd(&s), &icu_nfkd(&s));
    }
}

// ===========================================================================
// Proptest: 2000 cases -- Arabic/Hebrew combining
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    #[test]
    fn icu4x_arabic_hebrew_nfc(s in arabic_hebrew_combining_strategy()) {
        assert_eq_normalized("NFC-ar-he", &s, &our_nfc(&s), &icu_nfc(&s));
    }

    #[test]
    fn icu4x_arabic_hebrew_nfd(s in arabic_hebrew_combining_strategy()) {
        assert_eq_normalized("NFD-ar-he", &s, &our_nfd(&s), &icu_nfd(&s));
    }

    #[test]
    fn icu4x_arabic_hebrew_nfkc(s in arabic_hebrew_combining_strategy()) {
        assert_eq_normalized("NFKC-ar-he", &s, &our_nfkc(&s), &icu_nfkc(&s));
    }

    #[test]
    fn icu4x_arabic_hebrew_nfkd(s in arabic_hebrew_combining_strategy()) {
        assert_eq_normalized("NFKD-ar-he", &s, &our_nfkd(&s), &icu_nfkd(&s));
    }
}

// ===========================================================================
// Proptest: 2000 cases -- Indic (Devanagari) clusters
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    #[test]
    fn icu4x_indic_nfc(s in indic_cluster_strategy()) {
        assert_eq_normalized("NFC-indic", &s, &our_nfc(&s), &icu_nfc(&s));
    }

    #[test]
    fn icu4x_indic_nfd(s in indic_cluster_strategy()) {
        assert_eq_normalized("NFD-indic", &s, &our_nfd(&s), &icu_nfd(&s));
    }

    #[test]
    fn icu4x_indic_nfkc(s in indic_cluster_strategy()) {
        assert_eq_normalized("NFKC-indic", &s, &our_nfkc(&s), &icu_nfkc(&s));
    }

    #[test]
    fn icu4x_indic_nfkd(s in indic_cluster_strategy()) {
        assert_eq_normalized("NFKD-indic", &s, &our_nfkd(&s), &icu_nfkd(&s));
    }
}

// ===========================================================================
// Proptest: 1000 cases -- CJK + musical symbols
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn icu4x_cjk_musical_nfc(s in cjk_musical_strategy()) {
        assert_eq_normalized("NFC-cjk-music", &s, &our_nfc(&s), &icu_nfc(&s));
    }

    #[test]
    fn icu4x_cjk_musical_nfd(s in cjk_musical_strategy()) {
        assert_eq_normalized("NFD-cjk-music", &s, &our_nfd(&s), &icu_nfd(&s));
    }

    #[test]
    fn icu4x_cjk_musical_nfkc(s in cjk_musical_strategy()) {
        assert_eq_normalized("NFKC-cjk-music", &s, &our_nfkc(&s), &icu_nfkc(&s));
    }

    #[test]
    fn icu4x_cjk_musical_nfkd(s in cjk_musical_strategy()) {
        assert_eq_normalized("NFKD-cjk-music", &s, &our_nfkd(&s), &icu_nfkd(&s));
    }
}

// ===========================================================================
// Proptest: 1000 cases -- Boundary stress (SIMD chunk boundaries)
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn icu4x_boundary_nfc(s in boundary_stress_strategy()) {
        assert_eq_normalized("NFC-boundary", &s, &our_nfc(&s), &icu_nfc(&s));
    }

    #[test]
    fn icu4x_boundary_nfd(s in boundary_stress_strategy()) {
        assert_eq_normalized("NFD-boundary", &s, &our_nfd(&s), &icu_nfd(&s));
    }

    #[test]
    fn icu4x_boundary_nfkc(s in boundary_stress_strategy()) {
        assert_eq_normalized("NFKC-boundary", &s, &our_nfkc(&s), &icu_nfkc(&s));
    }

    #[test]
    fn icu4x_boundary_nfkd(s in boundary_stress_strategy()) {
        assert_eq_normalized("NFKD-boundary", &s, &our_nfkd(&s), &icu_nfkd(&s));
    }
}

// ===========================================================================
// Deterministic: Hangul edge cases
// ===========================================================================

#[test]
fn hangul_edge_cases() {
    let cases: &[(&str, &str)] = &[
        // Single L Jamo
        ("single-L", "\u{1100}"),
        // Single V Jamo
        ("single-V", "\u{1161}"),
        // Single T Jamo
        ("single-T", "\u{11A8}"),
        // L + V -> should compose to syllable GA (U+AC00)
        ("LV-compose", "\u{1100}\u{1161}"),
        // L + V + T -> should compose to syllable GAG (U+AC01)
        ("LVT-compose", "\u{1100}\u{1161}\u{11A8}"),
        // Precomposed LV syllable + T -> should compose to LVT
        ("LV+T-compose", "\u{AC00}\u{11A8}"),
        // Double L before V: second L+V composes, first L stays
        ("LL+V", "\u{1100}\u{1100}\u{1161}"),
        // L + V + V: L+V composes, second V stays (not a trailing consonant)
        ("L+VV", "\u{1100}\u{1161}\u{1175}"),
        // Triple Jamo: L + V + T + L + V
        ("triple-jamo", "\u{1100}\u{1161}\u{11A8}\u{1100}\u{1161}"),
        // Precomposed syllable followed by another L+V
        ("syllable+LV", "\u{AC00}\u{1100}\u{1161}"),
        // LVT syllable (precomposed) followed by trailing consonant: T should NOT merge
        ("LVT+T", "\u{AC01}\u{11A8}"),
        // Sequence of different precomposed syllables
        ("multi-syllable", "\u{AC00}\u{AC01}\u{D7A3}"),
        // All L jamo in sequence
        ("all-L", "\u{1100}\u{1101}\u{1102}\u{1103}\u{1104}"),
        // L+V with ASCII interspersed
        ("LV-ascii-LV", "\u{1100}\u{1161}A\u{1102}\u{1165}"),
        // Double-vowel combination: L + V + V (no composition of VV)
        ("double-vowel", "\u{1100}\u{1161}\u{1162}"),
        // Syllable at end of Hangul Syllables block
        ("last-syllable", "\u{D7A3}"),
        // Syllable at start of Hangul Syllables block
        ("first-syllable", "\u{AC00}"),
    ];

    for (label, input) in cases {
        let forms: &[(&str, fn(&str) -> String, fn(&str) -> String)] = &[
            ("NFC", our_nfc, icu_nfc),
            ("NFD", our_nfd, icu_nfd),
            ("NFKC", our_nfkc, icu_nfkc),
            ("NFKD", our_nfkd, icu_nfkd),
        ];

        for &(form_name, our_fn, icu_fn) in forms {
            let ours = our_fn(input);
            let reference = icu_fn(input);
            assert_eq!(
                ours, reference,
                "{form_name} Hangul edge case {label:?} failed\
                 \n  input code points: {input_cps}\
                 \n  ours  code points: {ours_cps}\
                 \n  ref   code points: {ref_cps}",
                form_name = form_name,
                label = label,
                input_cps = codepoints(input),
                ours_cps = codepoints(&ours),
                ref_cps = codepoints(&reference),
            );
        }
    }
}

// ===========================================================================
// Deterministic: NFKD long expansion
// ===========================================================================

#[test]
fn nfkd_long_expansion() {
    // U+FDFA: Arabic ligature "Sallallahou Alayhe Wasallam" -> 18-char expansion
    let fdfa = "\u{FDFA}";
    let fdfa_expected = "\u{0635}\u{0644}\u{0649} \u{0627}\u{0644}\u{0644}\u{0647} \
                         \u{0639}\u{0644}\u{064A}\u{0647} \u{0648}\u{0633}\u{0644}\u{0645}";

    let ours = our_nfkd(fdfa);
    let icu = icu_nfkd(fdfa);
    assert_eq!(
        ours, fdfa_expected,
        "NFKD U+FDFA: ours does not match expected 18-char expansion\
         \n  ours: {}\n  expected: {}",
        codepoints(&ours),
        codepoints(fdfa_expected),
    );
    assert_eq!(
        ours, icu,
        "NFKD U+FDFA: ours does not match icu4x\
         \n  ours: {}\n  icu:  {}",
        codepoints(&ours),
        codepoints(&icu),
    );

    // U+1D15E: musical symbol half note -> 2-char decomposition
    // Decomposes to U+1D157 (musical symbol void notehead) + U+1D165 (musical symbol combining stem)
    let note = "\u{1D15E}";
    let note_expected = "\u{1D157}\u{1D165}";

    let ours = our_nfkd(note);
    let icu = icu_nfkd(note);
    assert_eq!(
        ours, note_expected,
        "NFKD U+1D15E: ours does not match expected 2-char decomposition\
         \n  ours: {}\n  expected: {}",
        codepoints(&ours),
        codepoints(note_expected),
    );
    assert_eq!(
        ours, icu,
        "NFKD U+1D15E: ours does not match icu4x\
         \n  ours: {}\n  icu:  {}",
        codepoints(&ours),
        codepoints(&icu),
    );

    // Also test these in context with surrounding text
    let in_context = format!("Hello {} world {} end", fdfa, note);
    assert_eq_normalized(
        "NFKD-expansion-in-context",
        &in_context,
        &our_nfkd(&in_context),
        &icu_nfkd(&in_context),
    );
}

// ===========================================================================
// Deterministic: Triple reference check
//
// For each input, verify that simd_normalizer, icu_normalizer, AND
// unicode-normalization all agree. This catches the case where two wrong
// implementations happen to produce the same (incorrect) output.
// ===========================================================================

#[test]
fn triple_reference_check() {
    let cases: &[(&str, &str)] = &[
        // Empty
        ("empty", ""),
        // Pure ASCII
        ("ascii", "Hello, World!"),
        // Precomposed e-acute
        ("precomposed", "\u{00E9}"),
        // Decomposed e-acute
        ("decomposed", "\u{0065}\u{0301}"),
        // Hangul L+V composition
        ("hangul-lv", "\u{1100}\u{1161}"),
        // Hangul L+V+T composition
        ("hangul-lvt", "\u{1100}\u{1161}\u{11A8}"),
        // Precomposed Hangul syllable
        ("hangul-syllable", "\u{AC00}"),
        // CCC reorder: cedilla (ccc=202) + acute (ccc=230)
        ("ccc-reorder", "\u{0065}\u{0327}\u{0301}"),
        // Reversed CCC: acute (230) + cedilla (202)
        ("ccc-reversed", "\u{0065}\u{0301}\u{0327}"),
        // Ohm sign -> Omega
        ("ohm", "\u{2126}"),
        // Angstrom -> A-ring
        ("angstrom", "\u{212B}"),
        // Arabic ligature (NFKD long expansion)
        ("arabic-ligature", "\u{FDFA}"),
        // Musical note (composition exclusion)
        ("musical-note", "\u{1D15E}"),
        // fi ligature
        ("fi-ligature", "\u{FB01}"),
        // Devanagari cluster: ka + virama + sa + aa-matra
        ("devanagari-cluster", "\u{0915}\u{094D}\u{0938}\u{093E}"),
        // Arabic base + multiple marks
        ("arabic-marks", "\u{0628}\u{064E}\u{0651}"),
        // Hebrew letter + nikud
        ("hebrew-nikud", "\u{05D1}\u{05BC}\u{05B0}"),
        // Mixed Hangul: precomposed + jamo
        ("mixed-hangul", "\u{AC00}\u{1100}\u{1161}\u{11A8}"),
        // CJK compatibility ideograph (U+F900 -> U+8C48)
        ("cjk-compat", "\u{F900}"),
        // Parenthesized Hangul
        ("paren-hangul", "\u{320E}"),
        // Long combining sequence
        (
            "long-combining",
            "a\u{0300}\u{0301}\u{0302}\u{0303}\u{0304}\u{0305}\u{0306}\u{0307}",
        ),
        // Hiragana with dakuten
        ("hiragana-dakuten", "\u{304B}\u{3099}"),
        // Emoji (should be invariant)
        ("emoji", "\u{1F600}"),
        // Sinhala DDD edge case
        ("sinhala-ddd", "\u{0DDD}\u{0334}"),
    ];

    for (label, input) in cases {
        // NFC triple check
        assert_triple(
            &format!("NFC({label})"),
            input,
            our_nfc,
            icu_nfc,
            un_nfc,
        );

        // NFD triple check
        assert_triple(
            &format!("NFD({label})"),
            input,
            our_nfd,
            icu_nfd,
            un_nfd,
        );

        // NFKC triple check
        assert_triple(
            &format!("NFKC({label})"),
            input,
            our_nfkc,
            icu_nfkc,
            un_nfkc,
        );

        // NFKD triple check
        assert_triple(
            &format!("NFKD({label})"),
            input,
            our_nfkd,
            icu_nfkd,
            un_nfkd,
        );
    }
}
