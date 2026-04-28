//! Cross-vtable consistency tests for the aarch64 SIMD scanners.
//!
//! Component C of the 2026-04-28 aarch64 optimization design wires runtime
//! SVE2 detection into the dispatch layer. This test verifies that the NEON
//! and SVE2 backends produce byte-identical bitmasks on the same inputs, so
//! a faulty SVE2 path can never silently disagree with the NEON baseline.
//!
//! Coverage policy:
//! * Apple Silicon (M1/M2/M3/M4) does not implement SVE2 — the test detects
//!   this via `is_aarch64_feature_detected!("sve2")` and prints a skip
//!   message rather than failing. Real SVE2 verification (acceptance
//!   criteria item #4 in the spec) requires a Graviton 4 / Ampere
//!   AmpereOne / Neoverse-N2/V2/V3 host.
//! * The whole file is gated `#[cfg(target_arch = "aarch64")]`; on every
//!   other architecture this test compiles to nothing.

#![cfg(all(target_arch = "aarch64", feature = "std"))]

use proptest::prelude::*;
use proptest::test_runner::{Config as ProptestConfig, TestRunner};

/// NFC quickcheck threshold — same `bound` the production scanner uses to
/// flag bytes that need scalar fixup. Picking the same value gives the
/// proptest the same hit-density distribution the runtime sees.
const BOUND: u8 = 0xC0;

/// Strategy generating valid UTF-8 strings between 64 and 4096 bytes,
/// mixing BMP and supplementary-plane codepoints so the scanner's mask
/// bytes have a realistic density of hits.
fn utf8_string_strategy() -> impl Strategy<Value = String> {
    let ranges = prop::char::ranges(std::borrow::Cow::Borrowed(&[
        // ASCII (mostly below BOUND)
        '\u{0020}'..='\u{007E}',
        // Latin-1 supplement and Latin Extended (some bytes >= 0xC0)
        '\u{00A0}'..='\u{024F}',
        // Combining marks
        '\u{0300}'..='\u{036F}',
        // Greek + Cyrillic + Arabic + Devanagari
        '\u{0370}'..='\u{097F}',
        // Hangul Jamo
        '\u{1100}'..='\u{11FF}',
        // CJK
        '\u{4E00}'..='\u{9FFF}',
        // Hangul Syllables
        '\u{AC00}'..='\u{D7A3}',
        // Supplementary plane (4-byte UTF-8)
        '\u{1F300}'..='\u{1FAFF}',
        '\u{20000}'..='\u{2A6DF}',
    ]));
    // Aim for byte lengths in [64, 4096]. Worst case 4-byte chars: ~16..1024
    // codepoints. We pad to a minimum of 64 bytes after generation.
    prop::collection::vec(ranges, 16..1024).prop_map(|chars| chars.into_iter().collect::<String>())
}

/// Compare NEON and SVE2 `scan_chunk` over every aligned 64-byte chunk in
/// `bytes`. Returns the byte index of the first disagreement, or `None`.
unsafe fn first_disagreement(bytes: &[u8]) -> Option<usize> {
    let mut offset = 0usize;
    while offset + 64 <= bytes.len() {
        let ptr = unsafe { bytes.as_ptr().add(offset) };
        // SAFETY: caller guarantees SVE2 is supported on this host (test
        // checks `is_aarch64_feature_detected!`); both functions read 64
        // bytes from `ptr`, which is in-bounds.
        let neon_mask = unsafe { simd_normalizer::simd_test_api::neon_scan_chunk(ptr, BOUND) };
        let sve2_mask = unsafe { simd_normalizer::simd_test_api::sve2_scan_chunk(ptr, BOUND) };
        if neon_mask != sve2_mask {
            return Some(offset);
        }
        offset += 64;
    }
    None
}

#[test]
fn neon_and_sve2_agree_on_random_inputs() {
    if !std::arch::is_aarch64_feature_detected!("sve2") {
        eprintln!(
            "skipping neon_and_sve2_agree_on_random_inputs: host lacks SVE2 \
             (expected on Apple Silicon; needs Graviton 4 / Ampere AmpereOne \
             / Neoverse-N2/V2/V3 for real verification)"
        );
        return;
    }

    let mut runner = TestRunner::new(ProptestConfig::with_cases(1000));
    let strategy = utf8_string_strategy();
    runner
        .run(&strategy, |mut s| {
            // Pad to ensure at least 64 bytes of scannable input.
            while s.len() < 64 {
                s.push_str("aaaaaaaa");
            }
            // SAFETY: SVE2 was detected above.
            let disagreement = unsafe { first_disagreement(s.as_bytes()) };
            prop_assert!(
                disagreement.is_none(),
                "NEON and SVE2 disagreed at chunk offset {:?} on input of length {}",
                disagreement,
                s.len()
            );
            Ok(())
        })
        .expect("NEON and SVE2 vtables disagreed on a generated input");
}

#[test]
fn neon_and_sve2_agree_on_targeted_patterns() {
    if !std::arch::is_aarch64_feature_detected!("sve2") {
        eprintln!(
            "skipping neon_and_sve2_agree_on_targeted_patterns: host lacks \
             SVE2 (expected on Apple Silicon)"
        );
        return;
    }

    // Deterministic edge cases: each entry is a 64-byte slab pattern.
    let mut cases: Vec<[u8; 64]> = vec![
        [0x00; 64],      // all-clear
        [0xFF; 64],      // all-set
        [BOUND; 64],     // all-on-boundary
        [BOUND - 1; 64], // all-just-below
    ];
    // single-bit walks
    for i in 0..64 {
        let mut buf = [0u8; 64];
        buf[i] = 0xFF;
        cases.push(buf);
    }
    // alternating
    let mut alt = [0u8; 64];
    for (i, b) in alt.iter_mut().enumerate() {
        *b = if i % 2 == 0 { 0x00 } else { 0xFF };
    }
    cases.push(alt);

    for (idx, case) in cases.iter().enumerate() {
        // SAFETY: SVE2 detected; 64-byte buffer.
        let neon_mask =
            unsafe { simd_normalizer::simd_test_api::neon_scan_chunk(case.as_ptr(), BOUND) };
        let sve2_mask =
            unsafe { simd_normalizer::simd_test_api::sve2_scan_chunk(case.as_ptr(), BOUND) };
        assert_eq!(
            neon_mask, sve2_mask,
            "NEON/SVE2 disagreement on targeted case #{idx}: \
             neon=0x{neon_mask:016x} sve2=0x{sve2_mask:016x}"
        );
    }
}
