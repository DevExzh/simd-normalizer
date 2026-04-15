//! Criterion benchmarks for simd-normalizer vs unicode-normalization, icu4x, and simdutf8.
//!
//! Covers NFC, NFD, NFKC, NFKD normalization and is_normalized checks across
//! diverse Unicode input categories (~10 KB each).

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use icu_normalizer::{ComposingNormalizerBorrowed, DecomposingNormalizerBorrowed};

// ---------------------------------------------------------------------------
// Input generators (~10 KB each)
// ---------------------------------------------------------------------------

/// Pure ASCII text -- best case for SIMD passthrough.
fn gen_ascii_only() -> String {
    let base = "The quick brown fox jumps over the lazy dog. 0123456789!@#$%^&*() ";
    base.repeat(10_000 / base.len() + 1)[..10_000].to_string()
}

/// Western European accented text (Latin-1 Supplement).
fn gen_latin1() -> String {
    let base = "Héllo wörld! Ça fait plaisir de résumer l'été à Zürich. Ñoño año. ";
    let mut s = String::new();
    while s.len() < 10_000 {
        s.push_str(base);
    }
    s.truncate(s.floor_char_boundary(10_000));
    s
}

/// Chinese/Japanese/Korean ideographs.
fn gen_cjk() -> String {
    let base = "漢字仮名交じり文は日本語の表記に用いられる。中文也有很多汉字。한국어도 있습니다。";
    let mut s = String::new();
    while s.len() < 10_000 {
        s.push_str(base);
    }
    s.truncate(s.floor_char_boundary(10_000));
    s
}

/// Arabic text with diacritics (tashkeel).
fn gen_arabic() -> String {
    // Arabic with vowel marks (fathah, dammah, kasrah, shadda, sukun)
    let base = "بِسْمِ اللَّهِ الرَّحْمَٰنِ الرَّحِيمِ. الْحَمْدُ لِلَّهِ رَبِّ الْعَالَمِينَ. ";
    let mut s = String::new();
    while s.len() < 10_000 {
        s.push_str(base);
    }
    s.truncate(s.floor_char_boundary(10_000));
    s
}

/// Korean Hangul syllables (precomposed).
fn gen_hangul() -> String {
    let base = "대한민국헌법은국민의자유와권리를보장하며국가의안전보장과질서유지를위하여필요한경우에한하여법률로써제한할수있다";
    let mut s = String::new();
    while s.len() < 10_000 {
        s.push_str(base);
    }
    s.truncate(s.floor_char_boundary(10_000));
    s
}

/// Emoji with ZWJ sequences.
fn gen_emoji() -> String {
    let emojis = [
        "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}", // family
        "\u{1F469}\u{200D}\u{1F4BB}",                                   // woman technologist
        "\u{1F3F3}\u{FE0F}\u{200D}\u{1F308}",                           // rainbow flag
        "\u{1F468}\u{1F3FB}\u{200D}\u{2695}\u{FE0F}",                   // man health worker light
        "\u{1F1FA}\u{1F1F8}",                                           // US flag
        "\u{1F600}",
        "\u{1F60D}",
        "\u{1F680}",
        "\u{1F4A9}",
        "\u{2764}\u{FE0F}",
        "\u{1F44D}\u{1F3FD}", // thumbs up medium skin
    ];
    let mut s = String::new();
    let mut i = 0;
    while s.len() < 10_000 {
        s.push_str(emojis[i % emojis.len()]);
        s.push(' ');
        i += 1;
    }
    s.truncate(s.floor_char_boundary(10_000));
    s
}

/// Mix of all scripts.
fn gen_mixed() -> String {
    let segments = [
        "Hello world! ",
        "Héllo wörld! ",
        "漢字仮名交じり ",
        "بِسْمِ اللَّهِ ",
        "대한민국 ",
        "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467} ",
        "Ça fait été ",
        "日本語テスト ",
    ];
    let mut s = String::new();
    let mut i = 0;
    while s.len() < 10_000 {
        s.push_str(segments[i % segments.len()]);
        i += 1;
    }
    s.truncate(s.floor_char_boundary(10_000));
    s
}

/// Already NFC-normalized: precomposed characters (should be Cow::Borrowed fast path).
fn gen_already_nfc() -> String {
    // Use precomposed forms: é = U+00E9, ö = U+00F6, ü = U+00FC, etc.
    let base = "\u{00E9}\u{00F6}\u{00FC}\u{00E0}\u{00E8}\u{00F1}\u{00E7}\u{00C9}\u{00D6}\u{00DC}";
    let mut s = String::new();
    while s.len() < 10_000 {
        s.push_str(base);
    }
    s.truncate(s.floor_char_boundary(10_000));
    s
}

/// Worst case: base character + 30 combining marks (forces CCC sort).
fn gen_worst_case() -> String {
    let mut s = String::new();
    while s.len() < 10_000 {
        // Base character 'a'
        s.push('a');
        // 30 combining marks with varying CCC values to force sorting.
        // Use a mix of combining marks with different canonical combining classes:
        //   U+0300 (CCC=230) COMBINING GRAVE ACCENT
        //   U+0316 (CCC=220) COMBINING GRAVE ACCENT BELOW
        //   U+0327 (CCC=202) COMBINING CEDILLA
        //   U+0328 (CCC=202) COMBINING OGONEK
        //   U+0308 (CCC=230) COMBINING DIAERESIS
        //   U+0304 (CCC=230) COMBINING MACRON
        //   U+0301 (CCC=230) COMBINING ACUTE ACCENT
        //   U+030C (CCC=230) COMBINING CARON
        //   U+0323 (CCC=220) COMBINING DOT BELOW
        //   U+0330 (CCC=220) COMBINING TILDE BELOW
        let marks = [
            '\u{0300}', '\u{0316}', '\u{0327}', '\u{0308}', '\u{0301}', '\u{030C}', '\u{0323}',
            '\u{0330}', '\u{0304}', '\u{0328}',
        ];
        for i in 0..30 {
            s.push(marks[i % marks.len()]);
        }
    }
    s.truncate(s.floor_char_boundary(10_000));
    s
}

// ---------------------------------------------------------------------------
// Benchmark helpers -- reference crate wrappers
// ---------------------------------------------------------------------------

/// NFC via the reference crate (UFCS to avoid trait collision).
fn ref_nfc(data: &str) -> String {
    unicode_normalization::UnicodeNormalization::nfc(data).collect::<String>()
}

/// NFD via the reference crate.
fn ref_nfd(data: &str) -> String {
    unicode_normalization::UnicodeNormalization::nfd(data).collect::<String>()
}

/// NFKC via the reference crate.
fn ref_nfkc(data: &str) -> String {
    unicode_normalization::UnicodeNormalization::nfkc(data).collect::<String>()
}

/// NFKD via the reference crate.
fn ref_nfkd(data: &str) -> String {
    unicode_normalization::UnicodeNormalization::nfkd(data).collect::<String>()
}

// ---------------------------------------------------------------------------
// Benchmark helpers -- ICU4X normalizers (created once, reused)
// ---------------------------------------------------------------------------

// ICU4X normalizers are borrowed statics, so we just call them inline
// in each benchmark closure to avoid unnecessary String conversions.

// ---------------------------------------------------------------------------
// Input catalogue
// ---------------------------------------------------------------------------

struct InputCase {
    name: &'static str,
    data: String,
}

fn all_inputs() -> Vec<InputCase> {
    vec![
        InputCase {
            name: "ascii_only",
            data: gen_ascii_only(),
        },
        InputCase {
            name: "latin1",
            data: gen_latin1(),
        },
        InputCase {
            name: "cjk",
            data: gen_cjk(),
        },
        InputCase {
            name: "arabic",
            data: gen_arabic(),
        },
        InputCase {
            name: "hangul",
            data: gen_hangul(),
        },
        InputCase {
            name: "emoji",
            data: gen_emoji(),
        },
        InputCase {
            name: "mixed",
            data: gen_mixed(),
        },
        InputCase {
            name: "already_nfc",
            data: gen_already_nfc(),
        },
        InputCase {
            name: "worst_case",
            data: gen_worst_case(),
        },
    ]
}

// ---------------------------------------------------------------------------
// NFC benchmarks
// ---------------------------------------------------------------------------

fn bench_nfc(c: &mut Criterion) {
    let mut group = c.benchmark_group("nfc");
    let inputs = all_inputs();

    for input in &inputs {
        group.throughput(Throughput::Bytes(input.data.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("simd_normalizer", input.name),
            &input.data,
            |b, data| {
                b.iter(|| {
                    black_box(simd_normalizer::nfc().normalize(black_box(data)));
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("unicode_normalization", input.name),
            &input.data,
            |b, data| {
                b.iter(|| {
                    black_box(ref_nfc(black_box(data)));
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("icu4x", input.name),
            &input.data,
            |b, data| {
                let nfc = ComposingNormalizerBorrowed::new_nfc();
                b.iter(|| {
                    black_box(nfc.normalize(black_box(data)));
                });
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// NFD benchmarks
// ---------------------------------------------------------------------------

fn bench_nfd(c: &mut Criterion) {
    let mut group = c.benchmark_group("nfd");
    let inputs = all_inputs();

    for input in &inputs {
        group.throughput(Throughput::Bytes(input.data.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("simd_normalizer", input.name),
            &input.data,
            |b, data| {
                b.iter(|| {
                    black_box(simd_normalizer::nfd().normalize(black_box(data)));
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("unicode_normalization", input.name),
            &input.data,
            |b, data| {
                b.iter(|| {
                    black_box(ref_nfd(black_box(data)));
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("icu4x", input.name),
            &input.data,
            |b, data| {
                let nfd = DecomposingNormalizerBorrowed::new_nfd();
                b.iter(|| {
                    black_box(nfd.normalize(black_box(data)));
                });
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// NFKC benchmarks
// ---------------------------------------------------------------------------

fn bench_nfkc(c: &mut Criterion) {
    let mut group = c.benchmark_group("nfkc");
    let inputs = all_inputs();

    for input in &inputs {
        group.throughput(Throughput::Bytes(input.data.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("simd_normalizer", input.name),
            &input.data,
            |b, data| {
                b.iter(|| {
                    black_box(simd_normalizer::nfkc().normalize(black_box(data)));
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("unicode_normalization", input.name),
            &input.data,
            |b, data| {
                b.iter(|| {
                    black_box(ref_nfkc(black_box(data)));
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("icu4x", input.name),
            &input.data,
            |b, data| {
                let nfkc = ComposingNormalizerBorrowed::new_nfkc();
                b.iter(|| {
                    black_box(nfkc.normalize(black_box(data)));
                });
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// NFKD benchmarks
// ---------------------------------------------------------------------------

fn bench_nfkd(c: &mut Criterion) {
    let mut group = c.benchmark_group("nfkd");
    let inputs = all_inputs();

    for input in &inputs {
        group.throughput(Throughput::Bytes(input.data.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("simd_normalizer", input.name),
            &input.data,
            |b, data| {
                b.iter(|| {
                    black_box(simd_normalizer::nfkd().normalize(black_box(data)));
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("unicode_normalization", input.name),
            &input.data,
            |b, data| {
                b.iter(|| {
                    black_box(ref_nfkd(black_box(data)));
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("icu4x", input.name),
            &input.data,
            |b, data| {
                let nfkd = DecomposingNormalizerBorrowed::new_nfkd();
                b.iter(|| {
                    black_box(nfkd.normalize(black_box(data)));
                });
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// is_normalized benchmarks
// ---------------------------------------------------------------------------

fn bench_is_normalized(c: &mut Criterion) {
    let mut group = c.benchmark_group("is_normalized");
    let inputs = all_inputs();

    for input in &inputs {
        group.throughput(Throughput::Bytes(input.data.len() as u64));

        // --- is_nfc ---
        group.bench_with_input(
            BenchmarkId::new("simd_normalizer/is_nfc", input.name),
            &input.data,
            |b, data| {
                b.iter(|| {
                    black_box(simd_normalizer::nfc().is_normalized(black_box(data)));
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("unicode_normalization/is_nfc", input.name),
            &input.data,
            |b, data| {
                b.iter(|| {
                    black_box(unicode_normalization::is_nfc(black_box(data)));
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("icu4x/is_nfc", input.name),
            &input.data,
            |b, data| {
                let nfc = ComposingNormalizerBorrowed::new_nfc();
                b.iter(|| {
                    black_box(nfc.is_normalized(black_box(data)));
                });
            },
        );

        // --- is_nfd ---
        group.bench_with_input(
            BenchmarkId::new("simd_normalizer/is_nfd", input.name),
            &input.data,
            |b, data| {
                b.iter(|| {
                    black_box(simd_normalizer::nfd().is_normalized(black_box(data)));
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("unicode_normalization/is_nfd", input.name),
            &input.data,
            |b, data| {
                b.iter(|| {
                    black_box(unicode_normalization::is_nfd(black_box(data)));
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("icu4x/is_nfd", input.name),
            &input.data,
            |b, data| {
                let nfd = DecomposingNormalizerBorrowed::new_nfd();
                b.iter(|| {
                    black_box(nfd.is_normalized(black_box(data)));
                });
            },
        );

        // --- is_nfkc ---
        group.bench_with_input(
            BenchmarkId::new("simd_normalizer/is_nfkc", input.name),
            &input.data,
            |b, data| {
                b.iter(|| {
                    black_box(simd_normalizer::nfkc().is_normalized(black_box(data)));
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("unicode_normalization/is_nfkc", input.name),
            &input.data,
            |b, data| {
                b.iter(|| {
                    black_box(unicode_normalization::is_nfkc(black_box(data)));
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("icu4x/is_nfkc", input.name),
            &input.data,
            |b, data| {
                let nfkc = ComposingNormalizerBorrowed::new_nfkc();
                b.iter(|| {
                    black_box(nfkc.is_normalized(black_box(data)));
                });
            },
        );

        // --- is_nfkd ---
        group.bench_with_input(
            BenchmarkId::new("simd_normalizer/is_nfkd", input.name),
            &input.data,
            |b, data| {
                b.iter(|| {
                    black_box(simd_normalizer::nfkd().is_normalized(black_box(data)));
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("unicode_normalization/is_nfkd", input.name),
            &input.data,
            |b, data| {
                b.iter(|| {
                    black_box(unicode_normalization::is_nfkd(black_box(data)));
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("icu4x/is_nfkd", input.name),
            &input.data,
            |b, data| {
                let nfkd = DecomposingNormalizerBorrowed::new_nfkd();
                b.iter(|| {
                    black_box(nfkd.is_normalized(black_box(data)));
                });
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// normalize_to benchmarks (simd_normalizer only -- pre-allocated buffer)
// ---------------------------------------------------------------------------

fn bench_normalize_to(c: &mut Criterion) {
    let mut group = c.benchmark_group("normalize_to");
    let inputs = all_inputs();

    for input in &inputs {
        group.throughput(Throughput::Bytes(input.data.len() as u64));

        // NFC normalize_to
        group.bench_with_input(
            BenchmarkId::new("nfc", input.name),
            &input.data,
            |b, data| {
                let mut buf = String::with_capacity(data.len() * 2);
                b.iter(|| {
                    buf.clear();
                    black_box(simd_normalizer::nfc().normalize_to(black_box(data), &mut buf));
                });
            },
        );

        // NFD normalize_to
        group.bench_with_input(
            BenchmarkId::new("nfd", input.name),
            &input.data,
            |b, data| {
                let mut buf = String::with_capacity(data.len() * 2);
                b.iter(|| {
                    buf.clear();
                    black_box(simd_normalizer::nfd().normalize_to(black_box(data), &mut buf));
                });
            },
        );

        // NFKC normalize_to
        group.bench_with_input(
            BenchmarkId::new("nfkc", input.name),
            &input.data,
            |b, data| {
                let mut buf = String::with_capacity(data.len() * 2);
                b.iter(|| {
                    buf.clear();
                    black_box(simd_normalizer::nfkc().normalize_to(black_box(data), &mut buf));
                });
            },
        );

        // NFKD normalize_to
        group.bench_with_input(
            BenchmarkId::new("nfkd", input.name),
            &input.data,
            |b, data| {
                let mut buf = String::with_capacity(data.len() * 2);
                b.iter(|| {
                    buf.clear();
                    black_box(simd_normalizer::nfkd().normalize_to(black_box(data), &mut buf));
                });
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Case folding benchmarks
// ---------------------------------------------------------------------------

fn bench_casefold(c: &mut Criterion) {
    let mut group = c.benchmark_group("casefold");
    let inputs = all_inputs();

    for input in &inputs {
        group.throughput(Throughput::Bytes(input.data.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("standard", input.name),
            &input.data,
            |b, data| {
                b.iter(|| {
                    black_box(simd_normalizer::casefold(
                        black_box(data),
                        simd_normalizer::CaseFoldMode::Standard,
                    ));
                });
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Confusable skeleton benchmarks
// ---------------------------------------------------------------------------

fn bench_confusable(c: &mut Criterion) {
    let mut group = c.benchmark_group("confusable");
    let inputs = all_inputs();

    for input in &inputs {
        group.throughput(Throughput::Bytes(input.data.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("skeleton", input.name),
            &input.data,
            |b, data| {
                b.iter(|| {
                    black_box(simd_normalizer::skeleton(black_box(data)));
                });
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Matching pipeline benchmarks (fused vs sequential)
// ---------------------------------------------------------------------------

fn bench_matching(c: &mut Criterion) {
    let mut group = c.benchmark_group("matching");
    let inputs = all_inputs();
    let opts = simd_normalizer::matching::MatchingOptions::default();

    for input in &inputs {
        group.throughput(Throughput::Bytes(input.data.len() as u64));

        // Fused pipeline
        group.bench_with_input(
            BenchmarkId::new("normalize_for_matching", input.name),
            &input.data,
            |b, data| {
                b.iter(|| {
                    black_box(simd_normalizer::matching::normalize_for_matching(
                        black_box(data),
                        &opts,
                    ));
                });
            },
        );

        // Sequential: NFKC → casefold → skeleton (for comparison)
        group.bench_with_input(
            BenchmarkId::new("sequential_nfkc_fold_skel", input.name),
            &input.data,
            |b, data| {
                b.iter(|| {
                    let nfkc = simd_normalizer::nfkc().normalize(black_box(data));
                    let folded = simd_normalizer::casefold(
                        &nfkc,
                        simd_normalizer::CaseFoldMode::Standard,
                    );
                    black_box(simd_normalizer::skeleton(&folded));
                });
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// UTF-8 validation benchmarks (simdutf8 vs std)
// ---------------------------------------------------------------------------

fn bench_utf8_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("utf8_validation");
    let inputs = all_inputs();

    for input in &inputs {
        group.throughput(Throughput::Bytes(input.data.len() as u64));

        // simdutf8 basic (fastest, no error details)
        group.bench_with_input(
            BenchmarkId::new("simdutf8_basic", input.name),
            &input.data,
            |b, data| {
                let bytes = data.as_bytes();
                b.iter(|| {
                    let _ = black_box(simdutf8::basic::from_utf8(black_box(bytes)));
                });
            },
        );

        // simdutf8 compat (API-compatible with std, with error details)
        group.bench_with_input(
            BenchmarkId::new("simdutf8_compat", input.name),
            &input.data,
            |b, data| {
                let bytes = data.as_bytes();
                b.iter(|| {
                    let _ = black_box(simdutf8::compat::from_utf8(black_box(bytes)));
                });
            },
        );

        // std::str::from_utf8 (baseline)
        group.bench_with_input(
            BenchmarkId::new("std_from_utf8", input.name),
            &input.data,
            |b, data| {
                let bytes = data.as_bytes();
                b.iter(|| {
                    let _ = black_box(core::str::from_utf8(black_box(bytes)));
                });
            },
        );

        // simd_normalizer is_nfc as a scanning throughput reference
        group.bench_with_input(
            BenchmarkId::new("simd_normalizer_is_nfc", input.name),
            &input.data,
            |b, data| {
                b.iter(|| {
                    black_box(simd_normalizer::nfc().is_normalized(black_box(data)));
                });
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Criterion harness
// ---------------------------------------------------------------------------

criterion_group!(
    benches,
    bench_nfc,
    bench_nfd,
    bench_nfkc,
    bench_nfkd,
    bench_is_normalized,
    bench_normalize_to,
    bench_casefold,
    bench_confusable,
    bench_matching,
    bench_utf8_validation,
);
criterion_main!(benches);
