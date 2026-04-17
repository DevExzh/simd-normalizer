//! CLI-style tool that reads text from stdin (or uses a built-in demo) and
//! normalizes every line to a chosen Unicode form.
//!
//! This example demonstrates:
//! - Argument parsing for the normalization form (`--nfc`, `--nfd`, `--nfkc`, `--nfkd`)
//! - Buffered, line-by-line processing with `normalize_to()` for efficiency
//! - Before/after display with Rust debug escapes for non-printable and combining characters
//! - Detecting whether the input was already in the target form
//!
//! # Usage
//!
//! ```sh
//! # Demo mode (built-in sample strings, default NFC):
//! cargo run --example normalize_file
//! cargo run --example normalize_file -- --demo
//!
//! # Demo mode with a specific form:
//! cargo run --example normalize_file -- --nfkd --demo
//!
//! # Pipe text through stdin:
//! echo "cafe\u0301" | cargo run --example normalize_file
//! cat some_file.txt | cargo run --example normalize_file -- --nfkc
//! ```

use std::env;
use std::io::{self, BufRead, IsTerminal};

fn main() {
    // --- Parse CLI arguments ---------------------------------------------------
    let args: Vec<String> = env::args().collect();
    let (form, force_demo) = parse_args(&args);

    eprintln!("normalize_file: using {} normalization\n", form_name(form));

    // --- Decide input source ---------------------------------------------------
    // Use the built-in demo when:
    //   - `--demo` was passed explicitly, OR
    //   - stdin is a terminal (i.e. no data is being piped in).
    let stdin = io::stdin();
    let use_demo = force_demo || stdin.lock().is_terminal();

    if use_demo {
        eprintln!("(running built-in demo)\n");
        let demo = demo_strings();
        process_lines(demo.into_iter(), form);
    } else {
        // Piped input -- read line by line from stdin.
        let reader = stdin.lock();
        process_lines(reader.lines().map(|r| r.expect("failed to read stdin")), form);
    }
}

// ---------------------------------------------------------------------------
// Normalization form selection
// ---------------------------------------------------------------------------

/// Simple enum mirroring the four Unicode normalization forms.
#[derive(Clone, Copy)]
enum Form {
    Nfc,
    Nfd,
    Nfkc,
    Nfkd,
}

fn form_name(f: Form) -> &'static str {
    match f {
        Form::Nfc => "NFC",
        Form::Nfd => "NFD",
        Form::Nfkc => "NFKC",
        Form::Nfkd => "NFKD",
    }
}

fn parse_args(args: &[String]) -> (Form, bool) {
    let mut form = Form::Nfc;
    let mut demo = false;

    for arg in args.iter().skip(1) {
        match arg.as_str() {
            "--nfc" => form = Form::Nfc,
            "--nfd" => form = Form::Nfd,
            "--nfkc" => form = Form::Nfkc,
            "--nfkd" => form = Form::Nfkd,
            "--demo" => demo = true,
            "--help" | "-h" => {
                eprintln!("Usage: normalize_file [--nfc|--nfd|--nfkc|--nfkd] [--demo]");
                eprintln!("  Reads lines from stdin (or runs a built-in demo) and");
                eprintln!("  normalizes each line to the chosen Unicode form (default: NFC).");
                eprintln!();
                eprintln!("Options:");
                eprintln!("  --nfc   Canonical Decomposition, then Canonical Composition (default)");
                eprintln!("  --nfd   Canonical Decomposition");
                eprintln!("  --nfkc  Compatibility Decomposition, then Canonical Composition");
                eprintln!("  --nfkd  Compatibility Decomposition");
                eprintln!("  --demo  Run with built-in sample strings (ignores stdin)");
                std::process::exit(0);
            }
            other => {
                eprintln!("Unknown argument: {other}");
                eprintln!("Usage: normalize_file [--nfc|--nfd|--nfkc|--nfkd] [--demo]");
                std::process::exit(1);
            }
        }
    }
    (form, demo)
}

// ---------------------------------------------------------------------------
// Line processing
// ---------------------------------------------------------------------------

/// Normalize each line and print a before/after comparison.
///
/// Uses a single reusable `String` buffer via `normalize_to()` to avoid
/// allocating a new `String` for every line -- important in a real CLI tool
/// processing large files.
fn process_lines(lines: impl Iterator<Item = String>, form: Form) {
    // Create the normalizer once; reuse the output buffer across all lines.
    let mut buf = String::with_capacity(256);
    let mut count = 0u64;
    let mut already_normalized_count = 0u64;

    for line in lines {
        buf.clear();

        let was_normalized = normalize_to_form(&line, &mut buf, form);

        if was_normalized {
            already_normalized_count += 1;
        }
        count += 1;

        // Print with Rust debug escapes so combining characters and
        // non-printable code points are visible.
        println!("--- line {} {}", count, if was_normalized { "(already normalized)" } else { "(changed)" });
        println!("  input:  {:?}", line);
        println!("  output: {:?}", buf);
    }

    println!();
    println!(
        "Processed {} line(s): {} already in {}, {} changed.",
        count,
        already_normalized_count,
        form_name(form),
        count - already_normalized_count,
    );
}

/// Dispatch `normalize_to` to the correct normalizer type.
///
/// Because the four normalizer structs do not share a trait with `normalize_to`,
/// we dispatch manually.  The normalizers are zero-sized types, so construction
/// is free.
fn normalize_to_form(input: &str, out: &mut String, form: Form) -> bool {
    match form {
        Form::Nfc => simd_normalizer::nfc().normalize_to(input, out),
        Form::Nfd => simd_normalizer::nfd().normalize_to(input, out),
        Form::Nfkc => simd_normalizer::nfkc().normalize_to(input, out),
        Form::Nfkd => simd_normalizer::nfkd().normalize_to(input, out),
    }
}

// ---------------------------------------------------------------------------
// Built-in demo data
// ---------------------------------------------------------------------------

/// A curated set of Unicode strings that are interesting to normalize.
fn demo_strings() -> Vec<String> {
    vec![
        // Plain ASCII -- always already normalized in every form.
        "Hello, world!".into(),
        // Precomposed e-acute (U+00E9) -- NFC keeps it, NFD decomposes.
        "caf\u{00E9}".into(),
        // Decomposed e-acute: e + combining acute (U+0301) -- NFC composes, NFD keeps it.
        "caf\u{0065}\u{0301}".into(),
        // Multiple combining marks: o + combining tilde + combining acute.
        // Tests canonical ordering of combining characters.
        "\u{006F}\u{0303}\u{0301}".into(),
        // Hangul syllable GA (U+AC00) -- NFD decomposes to jamo L+V.
        "\u{AC00}".into(),
        // Hangul jamo sequence L+V+T -- NFC composes to a syllable.
        "\u{1100}\u{1161}\u{11A8}".into(),
        // fi ligature (U+FB01) -- only NFKC/NFKD decompose this.
        "of\u{FB01}ce".into(),
        // Fullwidth digits (U+FF10..U+FF19) -- NFKC/NFKD map to ASCII digits.
        "\u{FF11}\u{FF12}\u{FF13}".into(),
        // Superscript 2 (U+00B2) -- NFKC/NFKD map to plain "2".
        "x\u{00B2}".into(),
        // Greek with tonos: omicron + combining acute (U+0301).
        "\u{03BF}\u{0301}".into(),
        // Zero-width joiner sequences (emoji-style) -- should pass through.
        "a\u{200D}b".into(),
        // A longer mixed sentence with accented characters.
        "R\u{00E9}sum\u{00E9} for Na\u{00EF}ve Caf\u{00E9}".into(),
    ]
}
