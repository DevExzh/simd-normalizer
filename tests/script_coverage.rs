// tests/script_coverage.rs
//! Real-world text normalization tests for underrepresented scripts.
//!
//! Covers Georgian, Armenian, Ethiopic, Tibetan, Tamil, Telugu, Kannada,
//! Malayalam, and Myanmar with four test categories each:
//!   1. NFC/NFD round-trip
//!   2. Idempotence
//!   3. is_normalized consistency
//!   4. Differential vs icu_normalizer
//!
//! Scripts with combining marks (Tibetan, Ethiopic, Tamil) include
//! additional test strings that exercise CCC reordering.
//!
//! Run with: `cargo test --test script_coverage`

use std::borrow::Cow;
use std::fmt::Write;

// ---------------------------------------------------------------------------
// Our crate helpers (constructor API to avoid trait name collision)
// ---------------------------------------------------------------------------

fn our_nfc(s: &str) -> Cow<'_, str> {
    simd_normalizer::nfc().normalize(s)
}

fn our_nfd(s: &str) -> Cow<'_, str> {
    simd_normalizer::nfd().normalize(s)
}

fn our_is_nfc(s: &str) -> bool {
    simd_normalizer::nfc().is_normalized(s)
}

fn our_is_nfd(s: &str) -> bool {
    simd_normalizer::nfd().is_normalized(s)
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

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

/// Format codepoints for diagnostic output.
fn codepoint_dump(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        let _ = write!(out, "U+{:04X}", c as u32);
    }
    out
}

// ---------------------------------------------------------------------------
// Script test data
// ---------------------------------------------------------------------------

/// Each entry: (script_name, label, text).
/// Scripts with combining marks include extra entries exercising CCC reordering.
const SCRIPT_SAMPLES: &[(&str, &str, &str)] = &[
    // -----------------------------------------------------------------------
    // Georgian (Mkhedruli)
    // -----------------------------------------------------------------------
    (
        "Georgian",
        "sakartvelo",
        "\u{10E1}\u{10D0}\u{10E5}\u{10D0}\u{10E0}\u{10D7}\u{10D5}\u{10D4}\u{10DA}\u{10DD}",
    ), // საქართველო
    (
        "Georgian",
        "gamarjoba",
        "\u{10D2}\u{10D0}\u{10DB}\u{10D0}\u{10E0}\u{10EF}\u{10DD}\u{10D1}\u{10D0}",
    ), // გამარჯობა
    (
        "Georgian",
        "tbilisi",
        "\u{10D7}\u{10D1}\u{10D8}\u{10DA}\u{10D8}\u{10E1}\u{10D8}",
    ), // თბილისი
    (
        "Georgian",
        "mixed-sentence",
        "\u{10DB}\u{10D4} \u{10DB}\u{10D8}\u{10E7}\u{10D5}\u{10D0}\u{10E0}\u{10E1} \u{10E1}\u{10D0}\u{10E5}\u{10D0}\u{10E0}\u{10D7}\u{10D5}\u{10D4}\u{10DA}\u{10DD}",
    ), // მე მიყვარს საქართველო
    // -----------------------------------------------------------------------
    // Armenian
    // -----------------------------------------------------------------------
    (
        "Armenian",
        "hayastan",
        "\u{0540}\u{0561}\u{0575}\u{0561}\u{057D}\u{057F}\u{0561}\u{0576}",
    ), // Հայdelays -> Hayastan
    ("Armenian", "barev", "\u{0532}\u{0561}\u{0580}\u{0587}"), // Բարdelays -> Barev (U+0587 is ARMENIAN SMALL LIGATURE ECH YIWN)
    (
        "Armenian",
        "yerevan",
        "\u{0535}\u{0580}\u{0587}\u{0561}\u{0576}",
    ), // Երdelays -> Yerevan
    ("Armenian", "ech-yiwn-ligature", "\u{0587}"), // ew ligature (has NFKD decomposition)
    // -----------------------------------------------------------------------
    // Ethiopic / Ge'ez
    // -----------------------------------------------------------------------
    (
        "Ethiopic",
        "ethiopia",
        "\u{12A2}\u{1275}\u{12EE}\u{1335}\u{12EB}",
    ), // ኢትዮጵያ
    ("Ethiopic", "amharic-greeting", "\u{1230}\u{120B}\u{121D}"), // ሰላም (selam/peace)
    (
        "Ethiopic",
        "addis-ababa",
        "\u{12A0}\u{12F2}\u{1235} \u{12A0}\u{1260}\u{1263}",
    ), // አዲስ አበባ
    // Ethiopic combining marks: U+135D (ETHIOPIC COMBINING GEMINATION AND VOWEL LENGTH MARK),
    // U+135E (ETHIOPIC COMBINING VOWEL LENGTH MARK), U+135F (ETHIOPIC COMBINING GEMINATION MARK)
    ("Ethiopic", "combining-gemination", "\u{1200}\u{135F}"), // base + combining gemination mark
    ("Ethiopic", "combining-vowel-length", "\u{1200}\u{135E}"), // base + combining vowel length mark
    ("Ethiopic", "combining-both", "\u{1200}\u{135D}\u{135F}"), // base + two combining marks (CCC reordering test)
    (
        "Ethiopic",
        "multi-combining",
        "\u{1230}\u{135F}\u{120B}\u{135E}\u{121D}",
    ), // ሰ+gemination ላ+vowel-length ም
    // -----------------------------------------------------------------------
    // Tibetan
    // -----------------------------------------------------------------------
    ("Tibetan", "tibet", "\u{0F56}\u{0F7C}\u{0F51}"), // བོད (bod/Tibet)
    (
        "Tibetan",
        "tashi-delek",
        "\u{0F56}\u{0F40}\u{0FB2}\u{0F0B}\u{0F64}\u{0F72}\u{0F66}\u{0F0B}\u{0F56}\u{0F51}\u{0F7A}\u{0F0B}\u{0F63}\u{0F7A}\u{0F42}\u{0F66}",
    ), // bkra shis bde legs (Tashi Delek)
    // CCC reordering: KA + vowel sign U (CCC=132) + vowel sign AA (CCC=129)
    // Since 132 > 129, canonical ordering must reorder to CCC=129 first, then CCC=132
    ("Tibetan", "ccc-reorder-132-129", "\u{0F40}\u{0F74}\u{0F71}"),
    // CCC reordering: KA + vowel sign I (CCC=130) + vowel sign AA (CCC=129)
    // Since 130 > 129, canonical ordering must reorder
    ("Tibetan", "ccc-reorder-130-129", "\u{0F40}\u{0F72}\u{0F71}"),
    // Starter interruption: KA + vowel sign I (CCC=130) + sign rnam bcad (CCC=0, starter)
    (
        "Tibetan",
        "starter-interruption",
        "\u{0F40}\u{0F72}\u{0F7E}",
    ),
    // Multiple Tibetan combining marks with different CCC values
    // KA + vowel sign AA (CCC=129) + vowel sign U (CCC=132) -- already in order
    ("Tibetan", "ccc-already-ordered", "\u{0F40}\u{0F71}\u{0F74}"),
    // Three marks: KA + vowel sign U (CCC=132) + vowel sign I (CCC=130) + vowel sign AA (CCC=129)
    // All three need reordering: 132 > 130 > 129, should become 129 < 130 < 132
    (
        "Tibetan",
        "ccc-triple-reorder",
        "\u{0F40}\u{0F74}\u{0F72}\u{0F71}",
    ),
    // Tibetan mixed with subjoined consonants (CCC=0) and vowels
    (
        "Tibetan",
        "subjoined-mix",
        "\u{0F40}\u{0FB5}\u{0F71}\u{0F74}",
    ),
    // -----------------------------------------------------------------------
    // Tamil
    // -----------------------------------------------------------------------
    ("Tamil", "tamil", "\u{0BA4}\u{0BAE}\u{0BBF}\u{0BB4}\u{0BCD}"), // தமிழ் (Tamil)
    (
        "Tamil",
        "vanakkam",
        "\u{0BB5}\u{0BA3}\u{0B95}\u{0BCD}\u{0B95}\u{0BAE}\u{0BCD}",
    ), // வணக்கம் (Vanakkam/hello)
    (
        "Tamil",
        "chennai",
        "\u{0B9A}\u{0BC6}\u{0BA9}\u{0BCD}\u{0BA9}\u{0BC8}",
    ), // செனdelays -> Chennai
    // Tamil combining vowel signs: base + vowel sign II (U+0BC0, CCC=0)
    // and virama (U+0BCD, CCC=9) tests
    ("Tamil", "vowel-sign-ii", "\u{0B95}\u{0BC0}"), // கீ (KA + vowel sign II)
    ("Tamil", "virama-sequence", "\u{0B95}\u{0BCD}\u{0BB7}"), // க்� delay -> KA + virama + SSA (conjunct)
    // Tamil with nukta-like combining: AU length mark (U+0BD7, CCC=0) after base
    ("Tamil", "au-length-mark", "\u{0B95}\u{0BCA}\u{0BD7}"), // composite vowel sign
    // -----------------------------------------------------------------------
    // Telugu
    // -----------------------------------------------------------------------
    (
        "Telugu",
        "telugu",
        "\u{0C24}\u{0C46}\u{0C32}\u{0C41}\u{0C17}\u{0C41}",
    ), // తెలుగు (Telugu)
    (
        "Telugu",
        "hyderabad",
        "\u{0C39}\u{0C48}\u{0C26}\u{0C30}\u{0C3E}\u{0C2C}\u{0C3E}\u{0C26}\u{0C4D}",
    ), // హైదరాబాద్
    (
        "Telugu",
        "namaskaram",
        "\u{0C28}\u{0C2E}\u{0C38}\u{0C4D}\u{0C15}\u{0C3E}\u{0C30}\u{0C02}",
    ), // నమస్కారం
    ("Telugu", "vowel-signs", "\u{0C15}\u{0C46}\u{0C56}"), // KA + vowel sign E + AI length mark
    // -----------------------------------------------------------------------
    // Kannada
    // -----------------------------------------------------------------------
    (
        "Kannada",
        "kannada",
        "\u{0C95}\u{0CA8}\u{0CCD}\u{0CA8}\u{0CA1}",
    ), // ಕನ್ನdelays -> Kannada
    (
        "Kannada",
        "bengaluru",
        "\u{0CAC}\u{0CC6}\u{0C82}\u{0C97}\u{0CB3}\u{0CC2}\u{0CB0}\u{0CC1}",
    ), // ಬdelays -> Bengaluru
    (
        "Kannada",
        "namaskara",
        "\u{0CA8}\u{0CAE}\u{0CB8}\u{0CCD}\u{0C95}\u{0CBE}\u{0CB0}",
    ), // ನಮಸ್ಕಾdelays -> Namaskara
    ("Kannada", "vowel-signs", "\u{0C95}\u{0CC8}"), // ಕdelays -> KA + AI vowel sign
    // -----------------------------------------------------------------------
    // Malayalam
    // -----------------------------------------------------------------------
    (
        "Malayalam",
        "malayalam",
        "\u{0D2E}\u{0D32}\u{0D2F}\u{0D3E}\u{0D33}\u{0D02}",
    ), // മdelays -> Malayalam
    (
        "Malayalam",
        "namaskaram",
        "\u{0D28}\u{0D2E}\u{0D38}\u{0D4D}\u{0D15}\u{0D3E}\u{0D30}\u{0D02}",
    ), // നdelays -> Namaskaram
    (
        "Malayalam",
        "thiruvananthapuram",
        "\u{0D24}\u{0D3F}\u{0D30}\u{0D41}\u{0D35}\u{0D28}\u{0D28}\u{0D4D}\u{0D24}\u{0D2A}\u{0D41}\u{0D30}\u{0D02}",
    ), // തdelays -> Thiruvananthapuram
    ("Malayalam", "chillu-n", "\u{0D7B}"), // Chillu N (atomically encoded)
    // -----------------------------------------------------------------------
    // Myanmar / Burmese
    // -----------------------------------------------------------------------
    (
        "Myanmar",
        "myanmar",
        "\u{1019}\u{103C}\u{1014}\u{103A}\u{1019}\u{102C}",
    ), // မdelays -> Myanmar
    (
        "Myanmar",
        "mingalaba",
        "\u{1019}\u{1004}\u{103A}\u{1039}\u{1002}\u{101C}\u{102C}\u{1015}\u{102B}",
    ), // မdelays -> Mingalaba (hello)
    (
        "Myanmar",
        "yangon",
        "\u{101B}\u{1014}\u{103A}\u{1000}\u{102F}\u{1014}\u{103A}",
    ), // ရdelays -> Yangon
    // Myanmar has medial consonants (U+103B-U+103E) and vowel signs with specific ordering
    (
        "Myanmar",
        "medial-cluster",
        "\u{1000}\u{103C}\u{103D}\u{1031}\u{102C}",
    ), // complex onset cluster
];

// ---------------------------------------------------------------------------
// 1. NFC/NFD round-trip: nfc(nfd(text)) == nfc(text)
// ---------------------------------------------------------------------------

#[test]
fn script_round_trip_nfd_nfc() {
    let mut failures = Vec::new();

    for &(script, label, text) in SCRIPT_SAMPLES {
        let nfc_text = our_nfc(text);
        let nfd_text = our_nfd(text);
        let recovered = our_nfc(&nfd_text);

        if *recovered != *nfc_text {
            failures.push(format!(
                "  [{script}/{label}] NFC(NFD(text)) != NFC(text)\n\
                 \x20   input:     {input_cps}\n\
                 \x20   nfc:       {nfc_cps}\n\
                 \x20   nfd:       {nfd_cps}\n\
                 \x20   recovered: {recovered_cps}",
                input_cps = codepoint_dump(text),
                nfc_cps = codepoint_dump(&nfc_text),
                nfd_cps = codepoint_dump(&nfd_text),
                recovered_cps = codepoint_dump(&recovered),
            ));
        }
    }

    if !failures.is_empty() {
        panic!(
            "Round-trip NFC(NFD(text)) failures ({} total):\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
}

// ---------------------------------------------------------------------------
// 2. Idempotence: nfc(nfc(text)) == nfc(text) AND nfd(nfd(text)) == nfd(text)
// ---------------------------------------------------------------------------

#[test]
fn script_idempotence() {
    let mut failures = Vec::new();

    for &(script, label, text) in SCRIPT_SAMPLES {
        // NFC idempotence
        let nfc_once = our_nfc(text);
        let nfc_twice = our_nfc(&nfc_once);
        if *nfc_once != *nfc_twice {
            failures.push(format!(
                "  [{script}/{label}] NFC idempotence failed\n\
                 \x20   once:  {once_cps}\n\
                 \x20   twice: {twice_cps}",
                once_cps = codepoint_dump(&nfc_once),
                twice_cps = codepoint_dump(&nfc_twice),
            ));
        }

        // NFD idempotence
        let nfd_once = our_nfd(text);
        let nfd_twice = our_nfd(&nfd_once);
        if *nfd_once != *nfd_twice {
            failures.push(format!(
                "  [{script}/{label}] NFD idempotence failed\n\
                 \x20   once:  {once_cps}\n\
                 \x20   twice: {twice_cps}",
                once_cps = codepoint_dump(&nfd_once),
                twice_cps = codepoint_dump(&nfd_twice),
            ));
        }
    }

    if !failures.is_empty() {
        panic!(
            "Idempotence failures ({} total):\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
}

// ---------------------------------------------------------------------------
// 3. is_normalized consistency:
//    is_nfc(nfc(text)) == true AND is_nfd(nfd(text)) == true
// ---------------------------------------------------------------------------

#[test]
fn script_is_normalized_consistency() {
    let mut failures = Vec::new();

    for &(script, label, text) in SCRIPT_SAMPLES {
        // After NFC normalization, is_nfc must return true
        let nfc_text = our_nfc(text);
        if !our_is_nfc(&nfc_text) {
            failures.push(format!(
                "  [{script}/{label}] is_nfc(nfc(text)) returned false\n\
                 \x20   nfc: {nfc_cps}",
                nfc_cps = codepoint_dump(&nfc_text),
            ));
        }

        // After NFD normalization, is_nfd must return true
        let nfd_text = our_nfd(text);
        if !our_is_nfd(&nfd_text) {
            failures.push(format!(
                "  [{script}/{label}] is_nfd(nfd(text)) returned false\n\
                 \x20   nfd: {nfd_cps}",
                nfd_cps = codepoint_dump(&nfd_text),
            ));
        }
    }

    if !failures.is_empty() {
        panic!(
            "is_normalized consistency failures ({} total):\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
}

// ---------------------------------------------------------------------------
// 4. Differential vs icu_normalizer:
//    NFC and NFD output must match icu_normalizer
// ---------------------------------------------------------------------------

#[test]
fn script_differential_vs_icu_normalizer() {
    let mut failures = Vec::new();

    for &(script, label, text) in SCRIPT_SAMPLES {
        // NFC differential
        let our_nfc_result = our_nfc(text);
        let icu_nfc_result = icu_nfc(text);
        if *our_nfc_result != *icu_nfc_result {
            failures.push(format!(
                "  [{script}/{label}] NFC diverges from icu_normalizer\n\
                 \x20   input: {input_cps}\n\
                 \x20   ours:  {ours_cps}\n\
                 \x20   icu:   {icu_cps}",
                input_cps = codepoint_dump(text),
                ours_cps = codepoint_dump(&our_nfc_result),
                icu_cps = codepoint_dump(&icu_nfc_result),
            ));
        }

        // NFD differential
        let our_nfd_result = our_nfd(text);
        let icu_nfd_result = icu_nfd(text);
        if *our_nfd_result != *icu_nfd_result {
            failures.push(format!(
                "  [{script}/{label}] NFD diverges from icu_normalizer\n\
                 \x20   input: {input_cps}\n\
                 \x20   ours:  {ours_cps}\n\
                 \x20   icu:   {icu_cps}",
                input_cps = codepoint_dump(text),
                ours_cps = codepoint_dump(&our_nfd_result),
                icu_cps = codepoint_dump(&icu_nfd_result),
            ));
        }
    }

    if !failures.is_empty() {
        panic!(
            "Differential vs icu_normalizer failures ({} total):\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
}

// ---------------------------------------------------------------------------
// 5. Tibetan CCC reordering: focused tests
//
// Tibetan vowel marks have unusual CCC values:
//   U+0F71 vowel sign AA  -> CCC 129
//   U+0F72 vowel sign I   -> CCC 130
//   U+0F74 vowel sign U   -> CCC 132
//
// The canonical ordering algorithm must sort combining marks by CCC.
// These tests verify that our normalizer reorders correctly by comparing
// against icu_normalizer as the reference.
// ---------------------------------------------------------------------------

#[test]
fn tibetan_ccc_reordering_detailed() {
    let cases: &[(&str, &str)] = &[
        // CCC 132 before CCC 129 -> must reorder to 129, 132
        ("U-before-AA", "\u{0F40}\u{0F74}\u{0F71}"),
        // CCC 130 before CCC 129 -> must reorder to 129, 130
        ("I-before-AA", "\u{0F40}\u{0F72}\u{0F71}"),
        // CCC 132 before CCC 130 -> must reorder to 130, 132
        ("U-before-I", "\u{0F40}\u{0F74}\u{0F72}"),
        // Already ordered: CCC 129, 130, 132
        ("AA-I-U-ordered", "\u{0F40}\u{0F71}\u{0F72}\u{0F74}"),
        // Reverse order: CCC 132, 130, 129 -> must reorder to 129, 130, 132
        ("U-I-AA-reversed", "\u{0F40}\u{0F74}\u{0F72}\u{0F71}"),
        // Mixed with CCC 0 starter between combining marks
        // KA + vowel sign I (CCC=130) + TSEK (CCC=0) + KA + vowel sign AA (CCC=129)
        (
            "starter-between",
            "\u{0F40}\u{0F72}\u{0F0B}\u{0F40}\u{0F71}",
        ),
        // Single combining mark (no reordering needed)
        ("single-AA", "\u{0F40}\u{0F71}"),
        ("single-I", "\u{0F40}\u{0F72}"),
        ("single-U", "\u{0F40}\u{0F74}"),
        // Tibetan vowel marks mixed with sign rjes su nga ro (U+0F7E, CCC=0)
        // This tests that a CCC=0 character properly terminates a combining sequence
        ("ccc0-termination", "\u{0F40}\u{0F74}\u{0F71}\u{0F7E}"),
    ];

    for (label, input) in cases {
        let our_nfd_out = our_nfd(input);
        let icu_nfd_out = icu_nfd(input);
        assert_eq!(
            *our_nfd_out,
            icu_nfd_out,
            "Tibetan CCC reorder [{label}] NFD mismatch\n\
             \x20 input: {input_cps}\n\
             \x20 ours:  {ours_cps}\n\
             \x20 icu:   {icu_cps}",
            input_cps = codepoint_dump(input),
            ours_cps = codepoint_dump(&our_nfd_out),
            icu_cps = codepoint_dump(&icu_nfd_out),
        );

        let our_nfc_out = our_nfc(input);
        let icu_nfc_out = icu_nfc(input);
        assert_eq!(
            *our_nfc_out,
            icu_nfc_out,
            "Tibetan CCC reorder [{label}] NFC mismatch\n\
             \x20 input: {input_cps}\n\
             \x20 ours:  {ours_cps}\n\
             \x20 icu:   {icu_cps}",
            input_cps = codepoint_dump(input),
            ours_cps = codepoint_dump(&our_nfc_out),
            icu_cps = codepoint_dump(&icu_nfc_out),
        );
    }
}

// ---------------------------------------------------------------------------
// 6. Ethiopic combining marks: focused tests
// ---------------------------------------------------------------------------

#[test]
fn ethiopic_combining_marks_detailed() {
    let cases: &[(&str, &str)] = &[
        // U+135D ETHIOPIC COMBINING GEMINATION AND VOWEL LENGTH MARK (CCC=230)
        ("gemination-and-vowel", "\u{1200}\u{135D}"),
        // U+135E ETHIOPIC COMBINING VOWEL LENGTH MARK (CCC=230)
        ("vowel-length", "\u{1200}\u{135E}"),
        // U+135F ETHIOPIC COMBINING GEMINATION MARK (CCC=230)
        ("gemination", "\u{1200}\u{135F}"),
        // Two combining marks on same base (both CCC=230, stable sort)
        ("double-combining", "\u{1200}\u{135D}\u{135F}"),
        // Combining marks on different bases in sequence
        ("multi-base-combining", "\u{1200}\u{135F}\u{1201}\u{135E}"),
        // Ethiopic combining mark after Latin combining mark to test cross-script CCC
        ("cross-script-ccc", "A\u{0300}\u{135F}"),
    ];

    for (label, input) in cases {
        let our_nfd_out = our_nfd(input);
        let icu_nfd_out = icu_nfd(input);
        assert_eq!(
            *our_nfd_out,
            icu_nfd_out,
            "Ethiopic combining [{label}] NFD mismatch\n\
             \x20 input: {input_cps}\n\
             \x20 ours:  {ours_cps}\n\
             \x20 icu:   {icu_cps}",
            input_cps = codepoint_dump(input),
            ours_cps = codepoint_dump(&our_nfd_out),
            icu_cps = codepoint_dump(&icu_nfd_out),
        );

        let our_nfc_out = our_nfc(input);
        let icu_nfc_out = icu_nfc(input);
        assert_eq!(
            *our_nfc_out,
            icu_nfc_out,
            "Ethiopic combining [{label}] NFC mismatch\n\
             \x20 input: {input_cps}\n\
             \x20 ours:  {ours_cps}\n\
             \x20 icu:   {icu_cps}",
            input_cps = codepoint_dump(input),
            ours_cps = codepoint_dump(&our_nfc_out),
            icu_cps = codepoint_dump(&icu_nfc_out),
        );
    }
}

// ---------------------------------------------------------------------------
// 7. Tamil combining marks: focused tests
// ---------------------------------------------------------------------------

#[test]
fn tamil_combining_marks_detailed() {
    let cases: &[(&str, &str)] = &[
        // KA + virama (CCC=9)
        ("ka-virama", "\u{0B95}\u{0BCD}"),
        // KA + vowel sign I (CCC=0)
        ("ka-vowel-i", "\u{0B95}\u{0BBF}"),
        // KA + vowel sign II (CCC=0)
        ("ka-vowel-ii", "\u{0B95}\u{0BC0}"),
        // KA + vowel sign U (CCC=0)
        ("ka-vowel-u", "\u{0B95}\u{0BC1}"),
        // KA + virama + SSA (conjunct consonant)
        ("conjunct-ksha", "\u{0B95}\u{0BCD}\u{0BB7}"),
        // KA + virama + KA (geminated consonant)
        ("geminated-kka", "\u{0B95}\u{0BCD}\u{0B95}"),
        // Long sequence: KA + virama + KA + vowel sign AA
        ("long-kka-aa", "\u{0B95}\u{0BCD}\u{0B95}\u{0BBE}"),
        // Tamil OM sign (U+0BD0)
        ("om-sign", "\u{0BD0}"),
    ];

    for (label, input) in cases {
        let our_nfd_out = our_nfd(input);
        let icu_nfd_out = icu_nfd(input);
        assert_eq!(
            *our_nfd_out,
            icu_nfd_out,
            "Tamil combining [{label}] NFD mismatch\n\
             \x20 input: {input_cps}\n\
             \x20 ours:  {ours_cps}\n\
             \x20 icu:   {icu_cps}",
            input_cps = codepoint_dump(input),
            ours_cps = codepoint_dump(&our_nfd_out),
            icu_cps = codepoint_dump(&icu_nfd_out),
        );

        let our_nfc_out = our_nfc(input);
        let icu_nfc_out = icu_nfc(input);
        assert_eq!(
            *our_nfc_out,
            icu_nfc_out,
            "Tamil combining [{label}] NFC mismatch\n\
             \x20 input: {input_cps}\n\
             \x20 ours:  {ours_cps}\n\
             \x20 icu:   {icu_cps}",
            input_cps = codepoint_dump(input),
            ours_cps = codepoint_dump(&our_nfc_out),
            icu_cps = codepoint_dump(&icu_nfc_out),
        );
    }
}
