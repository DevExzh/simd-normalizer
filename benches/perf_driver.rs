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

fn main() {
    let workload = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: perf_driver <cjk|arabic|hangul|emoji|mixed>");
        std::process::exit(2);
    });

    let budget = parse_budget();

    let corpus = match workload.as_str() {
        "cjk" => gen_cjk(),
        other => {
            eprintln!("unknown workload: {other}");
            std::process::exit(2);
        }
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
