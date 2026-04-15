// tests/multilingual.rs
//! Multilingual real-world text normalization tests.
//!
//! Tests normalization correctness on real-world text files spanning
//! Arabic, Chinese, German, Emoji, French, Greek, Hebrew, Hindi,
//! Japanese (Hiragana, Katakana), Korean, Latin, Polish, Russian,
//! Spanish, Thai, and Turkish.
//!
//! Run with: `cargo test --test multilingual`

use std::borrow::Cow;
use std::fmt::Write;

// ---------------------------------------------------------------------------
// Text corpus: all benchmark data files, included at compile time
// ---------------------------------------------------------------------------

const TEXTS: &[(&str, &str)] = &[
    // ICU4X benchmark data (Carroll's Alice chapter 11 translations)
    (
        "arabic-carroll",
        include_str!("../3rdparty/icu4x/components/normalizer/benches/data/Carroll-11-ar.txt"),
    ),
    (
        "german-carroll",
        include_str!("../3rdparty/icu4x/components/normalizer/benches/data/Carroll-11-de.txt"),
    ),
    (
        "greek-carroll",
        include_str!("../3rdparty/icu4x/components/normalizer/benches/data/Carroll-11-el.txt"),
    ),
    (
        "spanish-carroll",
        include_str!("../3rdparty/icu4x/components/normalizer/benches/data/Carroll-11-es.txt"),
    ),
    (
        "french-carroll",
        include_str!("../3rdparty/icu4x/components/normalizer/benches/data/Carroll-11-fr.txt"),
    ),
    (
        "hebrew-carroll",
        include_str!("../3rdparty/icu4x/components/normalizer/benches/data/Carroll-11-he.txt"),
    ),
    (
        "polish-carroll",
        include_str!("../3rdparty/icu4x/components/normalizer/benches/data/Carroll-11-pl.txt"),
    ),
    (
        "russian-carroll",
        include_str!("../3rdparty/icu4x/components/normalizer/benches/data/Carroll-11-ru.txt"),
    ),
    (
        "thai-carroll",
        include_str!("../3rdparty/icu4x/components/normalizer/benches/data/Carroll-11-th.txt"),
    ),
    (
        "turkish-carroll",
        include_str!("../3rdparty/icu4x/components/normalizer/benches/data/Carroll-11-tr.txt"),
    ),
    // ICU4X benchmark data (name lists)
    (
        "japanese-hiragana",
        include_str!(
            "../3rdparty/icu4x/components/normalizer/benches/data/TestNames_Japanese_h.txt"
        ),
    ),
    (
        "japanese-katakana",
        include_str!(
            "../3rdparty/icu4x/components/normalizer/benches/data/TestNames_Japanese_k.txt"
        ),
    ),
    (
        "korean-names",
        include_str!("../3rdparty/icu4x/components/normalizer/benches/data/TestNames_Korean.txt"),
    ),
    (
        "latin-names",
        include_str!("../3rdparty/icu4x/components/normalizer/benches/data/TestNames_Latin.txt"),
    ),
    (
        "thai-names",
        include_str!("../3rdparty/icu4x/components/normalizer/benches/data/TestNames_Thai.txt"),
    ),
    (
        "english-wotw",
        include_str!("../3rdparty/icu4x/components/normalizer/benches/data/wotw.txt"),
    ),
    // simdutf8 benchmark data (lipsum texts)
    (
        "arabic-lipsum",
        include_str!("../3rdparty/simdutf8/bench/data/Arabic-Lipsum.txt"),
    ),
    (
        "chinese-lipsum",
        include_str!("../3rdparty/simdutf8/bench/data/Chinese-Lipsum.txt"),
    ),
    (
        "emoji-lipsum",
        include_str!("../3rdparty/simdutf8/bench/data/Emoji-Lipsum.txt"),
    ),
    (
        "hebrew-lipsum",
        include_str!("../3rdparty/simdutf8/bench/data/Hebrew-Lipsum.txt"),
    ),
    (
        "hindi-lipsum",
        include_str!("../3rdparty/simdutf8/bench/data/Hindi-Lipsum.txt"),
    ),
    (
        "japanese-lipsum",
        include_str!("../3rdparty/simdutf8/bench/data/Japanese-Lipsum.txt"),
    ),
    (
        "korean-lipsum",
        include_str!("../3rdparty/simdutf8/bench/data/Korean-Lipsum.txt"),
    ),
    (
        "latin-lipsum",
        include_str!("../3rdparty/simdutf8/bench/data/Latin-Lipsum.txt"),
    ),
    (
        "russian-lipsum",
        include_str!("../3rdparty/simdutf8/bench/data/Russian-Lipsum.txt"),
    ),
];

// ---------------------------------------------------------------------------
// Our crate helpers (constructor API to avoid trait name collision)
// ---------------------------------------------------------------------------

fn our_nfc(s: &str) -> Cow<'_, str> {
    simd_normalizer::nfc().normalize(s)
}

fn our_nfd(s: &str) -> Cow<'_, str> {
    simd_normalizer::nfd().normalize(s)
}

fn our_nfkc(s: &str) -> Cow<'_, str> {
    simd_normalizer::nfkc().normalize(s)
}

fn our_nfkd(s: &str) -> Cow<'_, str> {
    simd_normalizer::nfkd().normalize(s)
}

fn our_is_nfc(s: &str) -> bool {
    simd_normalizer::nfc().is_normalized(s)
}

fn our_is_nfd(s: &str) -> bool {
    simd_normalizer::nfd().is_normalized(s)
}

fn our_is_nfkc(s: &str) -> bool {
    simd_normalizer::nfkc().is_normalized(s)
}

fn our_is_nfkd(s: &str) -> bool {
    simd_normalizer::nfkd().is_normalized(s)
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
// unicode-normalization reference helpers
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

/// Format a short prefix of codepoints for diagnostic output.
/// Limits to at most 20 codepoints to avoid flooding the terminal.
fn codepoint_preview(s: &str, max: usize) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i >= max {
            let _ = write!(out, " ... ({} more)", s.chars().count() - max);
            break;
        }
        if i > 0 {
            out.push(' ');
        }
        let _ = write!(out, "U+{:04X}", c as u32);
    }
    out
}

/// Find the byte offset of the first divergence between two strings.
fn first_divergence(a: &str, b: &str) -> Option<usize> {
    a.bytes()
        .zip(b.bytes())
        .position(|(x, y)| x != y)
        .or_else(|| {
            if a.len() != b.len() {
                Some(a.len().min(b.len()))
            } else {
                None
            }
        })
}

// ---------------------------------------------------------------------------
// 1. Normalization Idempotence
//    For each text and each form: form(form(text)) == form(text)
// ---------------------------------------------------------------------------

#[test]
fn idempotence() {
    let mut failures = Vec::new();

    let forms: &[(&str, fn(&str) -> Cow<'_, str>)] = &[
        ("NFC", our_nfc),
        ("NFD", our_nfd),
        ("NFKC", our_nfkc),
        ("NFKD", our_nfkd),
    ];

    for &(name, text) in TEXTS {
        for &(form_name, form_fn) in forms {
            let once = form_fn(text);
            let twice = form_fn(&once);
            if *once != *twice {
                let diverge = first_divergence(&once, &twice);
                failures.push(format!(
                    "  [{name}] {form_name}: idempotence failed \
                     (once_len={}, twice_len={}, first_divergence_byte={:?})",
                    once.len(),
                    twice.len(),
                    diverge,
                ));
            }
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
// 2. Round-trip NFD -> NFC Recovery
//    If text is NFC: NFC(NFD(text)) == text
//    Otherwise:      NFC(NFD(text)) == NFC(text)
// ---------------------------------------------------------------------------

#[test]
fn round_trip_nfd_nfc() {
    let mut failures = Vec::new();

    for &(name, text) in TEXTS {
        let nfd_text = our_nfd(text);
        let recovered = our_nfc(&nfd_text);

        if our_is_nfc(text) {
            // Text is already NFC, so the round-trip should recover it exactly.
            if *recovered != *text {
                let diverge = first_divergence(&recovered, text);
                failures.push(format!(
                    "  [{name}] NFC(NFD(text)) != text (text is NFC) \
                     (text_len={}, recovered_len={}, first_divergence_byte={:?})",
                    text.len(),
                    recovered.len(),
                    diverge,
                ));
            }
        } else {
            // Text is not NFC, so round-trip should match NFC(text).
            let nfc_text = our_nfc(text);
            if *recovered != *nfc_text {
                let diverge = first_divergence(&recovered, &nfc_text);
                failures.push(format!(
                    "  [{name}] NFC(NFD(text)) != NFC(text) (text is not NFC) \
                     (nfc_len={}, recovered_len={}, first_divergence_byte={:?})",
                    nfc_text.len(),
                    recovered.len(),
                    diverge,
                ));
            }
        }
    }

    if !failures.is_empty() {
        panic!(
            "Round-trip NFD->NFC failures ({} total):\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
}

// ---------------------------------------------------------------------------
// 3. is_normalized Consistency
//    For each text and each form: if form(text) == text, then
//    is_form(text) must return true.
// ---------------------------------------------------------------------------

#[test]
fn is_normalized_consistency() {
    let mut failures = Vec::new();

    let forms: &[(&str, fn(&str) -> Cow<'_, str>, fn(&str) -> bool)] = &[
        ("NFC", our_nfc, our_is_nfc),
        ("NFD", our_nfd, our_is_nfd),
        ("NFKC", our_nfkc, our_is_nfkc),
        ("NFKD", our_nfkd, our_is_nfkd),
    ];

    for &(name, text) in TEXTS {
        for &(form_name, form_fn, is_fn) in forms {
            let normalized = form_fn(text);
            let text_is_unchanged = matches!(&normalized, Cow::Borrowed(_)) || *normalized == *text;

            if text_is_unchanged && !is_fn(text) {
                failures.push(format!(
                    "  [{name}] {form_name}: text is already normalized \
                     but is_{} returned false (text_len={})",
                    form_name.to_lowercase(),
                    text.len(),
                ));
            }

            // Also: normalized output must always report as normalized.
            if !is_fn(&normalized) {
                failures.push(format!(
                    "  [{name}] {form_name}: {form_name}(text) is not recognized \
                     as normalized by is_{} (normalized_len={})",
                    form_name.to_lowercase(),
                    normalized.len(),
                ));
            }
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
// 4. Differential: simd-normalizer vs icu_normalizer
// ---------------------------------------------------------------------------

#[test]
fn differential_vs_icu_normalizer() {
    let mut failures = Vec::new();

    let forms: &[(&str, fn(&str) -> Cow<'_, str>, fn(&str) -> String)] = &[
        ("NFC", our_nfc, icu_nfc),
        ("NFD", our_nfd, icu_nfd),
        ("NFKC", our_nfkc, icu_nfkc),
        ("NFKD", our_nfkd, icu_nfkd),
    ];

    for &(name, text) in TEXTS {
        for &(form_name, our_fn, icu_fn) in forms {
            let ours = our_fn(text);
            let reference = icu_fn(text);

            if *ours != *reference {
                let diverge = first_divergence(&ours, &reference);
                let ctx = diverge.map(|d| {
                    // Show codepoints around divergence point.
                    let start = ours[..d].chars().count().saturating_sub(2);
                    let ours_ctx = codepoint_preview(
                        &ours.chars().skip(start).take(10).collect::<String>(),
                        10,
                    );
                    let ref_ctx = codepoint_preview(
                        &reference.chars().skip(start).take(10).collect::<String>(),
                        10,
                    );
                    format!("ours_ctx=[{ours_ctx}] ref_ctx=[{ref_ctx}]")
                });
                failures.push(format!(
                    "  [{name}] {form_name}: diverges from icu_normalizer \
                     (ours_len={}, ref_len={}, first_divergence_byte={:?}, {})",
                    ours.len(),
                    reference.len(),
                    diverge,
                    ctx.as_deref().unwrap_or("identical length, internal diff"),
                ));
            }
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
// 5. Differential: simd-normalizer vs unicode-normalization
// ---------------------------------------------------------------------------

#[test]
fn differential_vs_unicode_normalization() {
    let mut failures = Vec::new();

    let forms: &[(&str, fn(&str) -> Cow<'_, str>, fn(&str) -> String)] = &[
        ("NFC", our_nfc, un_nfc),
        ("NFD", our_nfd, un_nfd),
        ("NFKC", our_nfkc, un_nfkc),
        ("NFKD", our_nfkd, un_nfkd),
    ];

    for &(name, text) in TEXTS {
        for &(form_name, our_fn, ref_fn) in forms {
            let ours = our_fn(text);
            let reference = ref_fn(text);

            if *ours != *reference {
                let diverge = first_divergence(&ours, &reference);
                let ctx = diverge.map(|d| {
                    let start = ours[..d].chars().count().saturating_sub(2);
                    let ours_ctx = codepoint_preview(
                        &ours.chars().skip(start).take(10).collect::<String>(),
                        10,
                    );
                    let ref_ctx = codepoint_preview(
                        &reference.chars().skip(start).take(10).collect::<String>(),
                        10,
                    );
                    format!("ours_ctx=[{ours_ctx}] ref_ctx=[{ref_ctx}]")
                });
                failures.push(format!(
                    "  [{name}] {form_name}: diverges from unicode-normalization \
                     (ours_len={}, ref_len={}, first_divergence_byte={:?}, {})",
                    ours.len(),
                    reference.len(),
                    diverge,
                    ctx.as_deref().unwrap_or("identical length, internal diff"),
                ));
            }
        }
    }

    if !failures.is_empty() {
        panic!(
            "Differential vs unicode-normalization failures ({} total):\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
}
