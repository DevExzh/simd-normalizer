//! Perf-counter driver binary.
//!
//! Usage: `target/release/perf_driver <workload>` — loops normalization for
//! ~2 s wall-clock, prints MB/s to stderr, and exits 0. Intended to be
//! wrapped with `perf stat`. See docs/superpowers/specs/2026-04-21-diag-perf-counters-design.md.

use std::hint::black_box;
use std::time::{Duration, Instant};

use simd_normalizer::nfc;

fn gen_cjk() -> String {
    let base = "漢字仮名交じり文は日本語の表記に用いられる。中文也有很多汉字。한국어도 있습니다。";
    let mut s = String::new();
    while s.len() < 10_000 {
        s.push_str(base);
    }
    s.truncate(s.floor_char_boundary(10_000));
    s
}

fn gen_arabic() -> String {
    let base = "بِسْمِ اللَّهِ الرَّحْمَٰنِ الرَّحِيمِ. الْحَمْدُ لِلَّهِ رَبِّ الْعَالَمِينَ. ";
    let mut s = String::new();
    while s.len() < 10_000 {
        s.push_str(base);
    }
    s.truncate(s.floor_char_boundary(10_000));
    s
}

fn gen_hangul() -> String {
    let base = "대한민국헌법은국민의자유와권리를보장하며국가의안전보장과질서유지를위하여필요한경우에한하여법률로써제한할수있다";
    let mut s = String::new();
    while s.len() < 10_000 {
        s.push_str(base);
    }
    s.truncate(s.floor_char_boundary(10_000));
    s
}

fn gen_emoji() -> String {
    let emojis = [
        "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}",
        "\u{1F469}\u{200D}\u{1F4BB}",
        "\u{1F3F3}\u{FE0F}\u{200D}\u{1F308}",
        "\u{1F468}\u{1F3FB}\u{200D}\u{2695}\u{FE0F}",
        "\u{1F1FA}\u{1F1F8}",
        "\u{1F600}",
        "\u{1F60D}",
        "\u{1F680}",
        "\u{1F4A9}",
        "\u{2764}\u{FE0F}",
        "\u{1F44D}\u{1F3FD}",
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

fn main() {
    let workload = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: perf_driver <cjk|arabic|hangul|emoji|mixed>");
        std::process::exit(2);
    });

    let budget = parse_budget();

    let corpus = match workload.as_str() {
        "cjk" => gen_cjk(),
        "arabic" => gen_arabic(),
        "hangul" => gen_hangul(),
        "emoji" => gen_emoji(),
        "mixed" => gen_mixed(),
        other => {
            eprintln!("unknown workload: {other}");
            std::process::exit(2);
        },
    };

    let normalizer = nfc();
    let mut bytes: u64 = 0;
    let start = Instant::now();
    while start.elapsed() < budget {
        let out = normalizer.normalize(black_box(&corpus));
        bytes = bytes.wrapping_add(out.len() as u64);
        black_box(&out);
    }
    let elapsed = start.elapsed();
    let mb = (bytes as f64) / 1_048_576.0;
    eprintln!(
        "workload={workload} bytes={bytes} elapsed_s={:.3} MB/s={:.1}",
        elapsed.as_secs_f64(),
        mb / elapsed.as_secs_f64()
    );
}

fn parse_budget() -> Duration {
    // Shortened by DIAG_SMOKE for CI plumbing tests.
    if std::env::var_os("DIAG_SMOKE").is_some() {
        Duration::from_millis(100)
    } else {
        Duration::from_secs(2)
    }
}
