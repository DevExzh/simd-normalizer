// tests/simd_boundaries.rs
//! SIMD chunk boundary stress tests.
//!
//! The SIMD normalizer processes 64-byte chunks.  These tests systematically
//! exercise inputs whose lengths land exactly at chunk boundaries (64, 128
//! bytes) and place multi-byte UTF-8 characters so they *straddle* those
//! boundaries.  This catches off-by-one bugs in chunk scanning, mask walking,
//! and the hand-off between SIMD passthrough and scalar decode paths.

extern crate alloc;

use alloc::borrow::Cow;
use alloc::string::String;

use simd_normalizer::normalizer::{NfcNormalizer, NfdNormalizer, NfkcNormalizer, NfkdNormalizer};

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
// Helpers
// ---------------------------------------------------------------------------

#[allow(clippy::ptr_arg)]
fn is_borrowed(cow: &Cow<'_, str>) -> bool {
    matches!(cow, Cow::Borrowed(_))
}

/// Assert all 4 forms match ICU4X for the given input.
fn assert_all_forms_match_icu(input: &str) {
    let nfc_result = NfcNormalizer.normalize(input);
    let nfd_result = NfdNormalizer.normalize(input);
    let nfkc_result = NfkcNormalizer.normalize(input);
    let nfkd_result = NfkdNormalizer.normalize(input);

    let icu_nfc_result = icu_nfc(input);
    let icu_nfd_result = icu_nfd(input);
    let icu_nfkc_result = icu_nfkc(input);
    let icu_nfkd_result = icu_nfkd(input);

    assert_eq!(
        &*nfc_result,
        &icu_nfc_result,
        "NFC mismatch for input of {} bytes",
        input.len()
    );
    assert_eq!(
        &*nfd_result,
        &icu_nfd_result,
        "NFD mismatch for input of {} bytes",
        input.len()
    );
    assert_eq!(
        &*nfkc_result,
        &icu_nfkc_result,
        "NFKC mismatch for input of {} bytes",
        input.len()
    );
    assert_eq!(
        &*nfkd_result,
        &icu_nfkd_result,
        "NFKD mismatch for input of {} bytes",
        input.len()
    );
}

/// Build a string of exactly `n` ASCII bytes using a repeating pattern.
fn ascii_bytes(n: usize) -> String {
    // Use printable ASCII that is stable under all normalization forms.
    let pattern = b"abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ_-";
    let mut s = String::with_capacity(n);
    for i in 0..n {
        s.push(pattern[i % pattern.len()] as char);
    }
    assert_eq!(s.len(), n);
    s
}

// ============================================================================
// 1. Exactly 64 ASCII bytes -- one full SIMD chunk
// ============================================================================

#[test]
fn exactly_64_ascii_bytes_all_forms() {
    let input = ascii_bytes(64);
    assert_eq!(input.len(), 64);

    // All 4 forms should return Borrowed (pure ASCII is already normalized).
    let nfc = NfcNormalizer.normalize(&input);
    let nfd = NfdNormalizer.normalize(&input);
    let nfkc = NfkcNormalizer.normalize(&input);
    let nfkd = NfkdNormalizer.normalize(&input);

    assert!(is_borrowed(&nfc), "NFC should borrow 64-byte ASCII");
    assert!(is_borrowed(&nfd), "NFD should borrow 64-byte ASCII");
    assert!(is_borrowed(&nfkc), "NFKC should borrow 64-byte ASCII");
    assert!(is_borrowed(&nfkd), "NFKD should borrow 64-byte ASCII");

    assert_eq!(&*nfc, &input);
    assert_eq!(&*nfd, &input);
    assert_eq!(&*nfkc, &input);
    assert_eq!(&*nfkd, &input);

    // Differential check against ICU4X.
    assert_all_forms_match_icu(&input);
}

// ============================================================================
// 2. Exactly 128 ASCII bytes -- two full SIMD chunks
// ============================================================================

#[test]
fn exactly_128_ascii_bytes_all_forms() {
    let input = ascii_bytes(128);
    assert_eq!(input.len(), 128);

    let nfc = NfcNormalizer.normalize(&input);
    let nfd = NfdNormalizer.normalize(&input);
    let nfkc = NfkcNormalizer.normalize(&input);
    let nfkd = NfkdNormalizer.normalize(&input);

    assert!(is_borrowed(&nfc), "NFC should borrow 128-byte ASCII");
    assert!(is_borrowed(&nfd), "NFD should borrow 128-byte ASCII");
    assert!(is_borrowed(&nfkc), "NFKC should borrow 128-byte ASCII");
    assert!(is_borrowed(&nfkd), "NFKD should borrow 128-byte ASCII");

    assert_eq!(&*nfc, &input);
    assert_eq!(&*nfd, &input);
    assert_eq!(&*nfkc, &input);
    assert_eq!(&*nfkd, &input);

    assert_all_forms_match_icu(&input);
}

// ============================================================================
// 3. 63 ASCII bytes + 1 two-byte character
//    Non-ASCII char starts at byte 63, straddling the 64-byte boundary.
// ============================================================================

#[test]
fn ascii_63_plus_two_byte_char_straddling_64() {
    // U+00E9 = 'e' with acute accent, encoded as 0xC3 0xA9 (2 bytes).
    // Byte layout: [0..62] ASCII + [63..64] two-byte char = 65 bytes total.
    let prefix = ascii_bytes(63);
    let input = format!("{}\u{00E9}", prefix);
    assert_eq!(input.len(), 65, "63 ASCII + 2-byte char = 65 bytes");

    assert_all_forms_match_icu(&input);
}

#[test]
fn ascii_63_plus_two_byte_char_nfc_stable() {
    // U+00FC (u with diaeresis) is NFC-stable (precomposed).
    let prefix = ascii_bytes(63);
    let input = format!("{}\u{00FC}", prefix);
    assert_eq!(input.len(), 65);

    let nfc = NfcNormalizer.normalize(&input);
    assert_eq!(&*nfc, &input, "NFC should be identity for precomposed char");

    assert_all_forms_match_icu(&input);
}

// ============================================================================
// 4. 64 ASCII bytes + non-ASCII trailing
//    Boundary exactly at start of second chunk.
// ============================================================================

#[test]
fn ascii_64_plus_non_ascii_trailing() {
    // Non-ASCII starts at byte 64 (the first byte of the second chunk).
    let prefix = ascii_bytes(64);

    // U+00E9 (e-acute, 2 bytes)
    let input = format!("{}\u{00E9}", prefix);
    assert_eq!(input.len(), 66);
    assert_all_forms_match_icu(&input);

    // U+4E00 (CJK, 3 bytes)
    let input_cjk = format!("{}\u{4E00}", prefix);
    assert_eq!(input_cjk.len(), 67);
    assert_all_forms_match_icu(&input_cjk);

    // U+1F600 (emoji, 4 bytes)
    let input_emoji = format!("{}\u{1F600}", prefix);
    assert_eq!(input_emoji.len(), 68);
    assert_all_forms_match_icu(&input_emoji);
}

#[test]
fn ascii_64_plus_combining_sequence_trailing() {
    // Combining sequence starts right at byte 64.
    let prefix = ascii_bytes(64);
    // A + combining grave (U+0300) + combining acute (U+0301)
    let input = format!("{}A\u{0300}\u{0301}", prefix);
    // 64 + 1 (A) + 2 (U+0300) + 2 (U+0301) = 69 bytes
    assert_eq!(input.len(), 69);
    assert_all_forms_match_icu(&input);
}

// ============================================================================
// 5. Multi-byte characters straddling byte offset 63-64
// ============================================================================

#[test]
fn cjk_3byte_straddling_64_boundary() {
    // 62 ASCII bytes + one 3-byte character (U+4E00) at bytes 62-64.
    let prefix = ascii_bytes(62);
    let input = format!("{}\u{4E00}", prefix);
    assert_eq!(input.len(), 65, "62 ASCII + 3-byte CJK = 65 bytes");
    assert_all_forms_match_icu(&input);
}

#[test]
fn emoji_4byte_straddling_64_boundary() {
    // 61 ASCII bytes + one 4-byte character (U+1F600) at bytes 61-64.
    let prefix = ascii_bytes(61);
    let input = format!("{}\u{1F600}", prefix);
    assert_eq!(input.len(), 65, "61 ASCII + 4-byte emoji = 65 bytes");
    assert_all_forms_match_icu(&input);
}

#[test]
fn combining_sequence_starting_at_byte_60() {
    // 60 ASCII bytes + A (1 byte) + combining grave U+0300 (2 bytes) +
    // combining acute U+0301 (2 bytes) = 65 bytes.
    // The combining sequence starts at byte 60 and crosses 64.
    let prefix = ascii_bytes(60);
    let input = format!("{}A\u{0300}\u{0301}", prefix);
    assert_eq!(input.len(), 65, "60 + 1 + 2 + 2 = 65 bytes");
    assert_all_forms_match_icu(&input);
}

#[test]
fn combining_sequence_starting_at_byte_62() {
    // 62 ASCII bytes + A (1 byte) + combining grave U+0300 (2 bytes) = 65 bytes.
    // A starts at byte 62, combining mark at 63-64 -- straddles the boundary.
    let prefix = ascii_bytes(62);
    let input = format!("{}A\u{0300}", prefix);
    assert_eq!(input.len(), 65, "62 + 1 + 2 = 65 bytes");
    assert_all_forms_match_icu(&input);
}

// ============================================================================
// 6. Same straddle tests at byte offset 127-128 (two-chunk boundary)
// ============================================================================

#[test]
fn cjk_3byte_straddling_128_boundary() {
    // 126 ASCII bytes + one 3-byte character (U+4E00) at bytes 126-128.
    let prefix = ascii_bytes(126);
    let input = format!("{}\u{4E00}", prefix);
    assert_eq!(input.len(), 129, "126 ASCII + 3-byte CJK = 129 bytes");
    assert_all_forms_match_icu(&input);
}

#[test]
fn emoji_4byte_straddling_128_boundary() {
    // 125 ASCII bytes + one 4-byte character (U+1F600) at bytes 125-128.
    let prefix = ascii_bytes(125);
    let input = format!("{}\u{1F600}", prefix);
    assert_eq!(input.len(), 129, "125 ASCII + 4-byte emoji = 129 bytes");
    assert_all_forms_match_icu(&input);
}

#[test]
fn two_byte_char_straddling_128_boundary() {
    // 127 ASCII bytes + one 2-byte character at bytes 127-128.
    let prefix = ascii_bytes(127);
    let input = format!("{}\u{00E9}", prefix);
    assert_eq!(input.len(), 129, "127 ASCII + 2-byte char = 129 bytes");
    assert_all_forms_match_icu(&input);
}

#[test]
fn combining_sequence_straddling_128_boundary() {
    // 126 ASCII bytes + A (1 byte at 126) + combining grave U+0300 (2 bytes at 127-128).
    let prefix = ascii_bytes(126);
    let input = format!("{}A\u{0300}", prefix);
    assert_eq!(input.len(), 129, "126 + 1 + 2 = 129 bytes");
    assert_all_forms_match_icu(&input);
}

// ============================================================================
// 7. Combining sequence crossing chunk boundary
// ============================================================================

#[test]
fn a_ring_decomposes_across_64_boundary() {
    // U+00C5 (A-ring) is 2 bytes (0xC3 0x85) and decomposes in NFD to
    // A (U+0041) + combining ring above (U+030A).
    // Place it at bytes 62-63, so the decomposition products span the boundary.
    let prefix = ascii_bytes(62);
    let input = format!("{}\u{00C5}", prefix);
    assert_eq!(input.len(), 64, "62 ASCII + 2-byte A-ring = 64 bytes");
    assert_all_forms_match_icu(&input);
}

#[test]
fn a_ring_plus_combining_marks_in_next_chunk() {
    // U+00C5 at bytes 62-63, then combining marks in the next chunk.
    let prefix = ascii_bytes(62);
    // A-ring (2 bytes) + combining acute (U+0301, 2 bytes) + combining cedilla (U+0327, 2 bytes)
    let input = format!("{}\u{00C5}\u{0301}\u{0327}", prefix);
    assert_eq!(input.len(), 68, "62 + 2 + 2 + 2 = 68 bytes");
    assert_all_forms_match_icu(&input);
}

#[test]
fn starter_before_64_combining_after_64() {
    // A starter right before byte 64, combining marks right after byte 64.
    // 63 ASCII bytes + 'A' (byte 63) = 64 bytes, then combining marks in next chunk.
    let prefix = ascii_bytes(63);
    // A at byte 63, then combining grave (2 bytes) + combining acute (2 bytes) at bytes 64-67.
    let input = format!("{}A\u{0300}\u{0301}", prefix);
    assert_eq!(input.len(), 68, "63 + 1 + 2 + 2 = 68 bytes");
    assert_all_forms_match_icu(&input);
}

#[test]
fn starter_at_byte_63_long_combining_run() {
    // 63 ASCII + 'o' at byte 63 + multiple combining marks crossing into the next chunk.
    let prefix = ascii_bytes(63);
    // o + combining tilde (U+0303) + combining macron (U+0304) + combining dot below (U+0323)
    let input = format!("{}o\u{0303}\u{0304}\u{0323}", prefix);
    assert_eq!(input.len(), 70, "63 + 1 + 2 + 2 + 2 = 70 bytes");
    assert_all_forms_match_icu(&input);
}

#[test]
fn decomposing_char_at_boundary_with_trailing_combiners() {
    // Place a character that decomposes (U+01FA, A-ring-acute, decomposes to
    // A + ring + acute) right before the 64-byte boundary.
    // U+01FA = 0xC7 0xBA (2 bytes).
    let prefix = ascii_bytes(62);
    let input = format!("{}\u{01FA}", prefix);
    assert_eq!(input.len(), 64, "62 + 2 = 64 bytes");
    assert_all_forms_match_icu(&input);
}

#[test]
fn hangul_syllable_straddling_64_boundary() {
    // Hangul syllable U+AC00 (3 bytes, decomposes in NFD to L+V+T jamo).
    // Place at bytes 62-64.
    let prefix = ascii_bytes(62);
    let input = format!("{}\u{AC00}", prefix);
    assert_eq!(input.len(), 65, "62 + 3 = 65 bytes");
    assert_all_forms_match_icu(&input);
}

// ============================================================================
// 8. Large input (1 MB) of repeated pattern
// ============================================================================

#[test]
fn large_1mb_repeating_pattern() {
    // Pattern: 50 ASCII bytes + "日本語" (9 bytes CJK) + U+1F600 (4 bytes emoji)
    // = 63 bytes per cycle.
    let ascii_part = ascii_bytes(50);
    let pattern = format!("{}{}{}", ascii_part, "日本語", "\u{1F600}");
    assert_eq!(pattern.len(), 63, "50 + 9 + 4 = 63 bytes per cycle");

    // Repeat to fill ~1MB.
    let target_size = 1024 * 1024; // 1 MB
    let repeats = target_size / pattern.len() + 1;
    let large_input: String = pattern.repeat(repeats);
    assert!(
        large_input.len() >= target_size,
        "input should be >= 1MB, got {} bytes",
        large_input.len()
    );

    // Verify all 4 forms match ICU4X.
    let our_nfc = NfcNormalizer.normalize(&large_input);
    let our_nfd = NfdNormalizer.normalize(&large_input);
    let our_nfkc = NfkcNormalizer.normalize(&large_input);
    let our_nfkd = NfkdNormalizer.normalize(&large_input);

    let ref_nfc = icu_nfc(&large_input);
    let ref_nfd = icu_nfd(&large_input);
    let ref_nfkc = icu_nfkc(&large_input);
    let ref_nfkd = icu_nfkd(&large_input);

    assert_eq!(
        &*our_nfc,
        &ref_nfc,
        "NFC mismatch on 1MB input ({} bytes)",
        large_input.len()
    );
    assert_eq!(
        &*our_nfd,
        &ref_nfd,
        "NFD mismatch on 1MB input ({} bytes)",
        large_input.len()
    );
    assert_eq!(
        &*our_nfkc,
        &ref_nfkc,
        "NFKC mismatch on 1MB input ({} bytes)",
        large_input.len()
    );
    assert_eq!(
        &*our_nfkd,
        &ref_nfkd,
        "NFKD mismatch on 1MB input ({} bytes)",
        large_input.len()
    );
}

// ============================================================================
// 9. Exhaustive boundary sweep around 64 and 128
//    (tests every length from 60..=68 and 124..=132)
// ============================================================================

#[test]
fn boundary_sweep_ascii_around_64() {
    for len in 60..=68 {
        let input = ascii_bytes(len);
        let nfc = NfcNormalizer.normalize(&input);
        let nfd = NfdNormalizer.normalize(&input);
        let nfkc = NfkcNormalizer.normalize(&input);
        let nfkd = NfkdNormalizer.normalize(&input);

        assert!(is_borrowed(&nfc), "NFC should borrow {}-byte ASCII", len);
        assert!(is_borrowed(&nfd), "NFD should borrow {}-byte ASCII", len);
        assert!(is_borrowed(&nfkc), "NFKC should borrow {}-byte ASCII", len);
        assert!(is_borrowed(&nfkd), "NFKD should borrow {}-byte ASCII", len);

        assert_eq!(&*nfc, &input);
        assert_all_forms_match_icu(&input);
    }
}

#[test]
fn boundary_sweep_ascii_around_128() {
    for len in 124..=132 {
        let input = ascii_bytes(len);
        let nfc = NfcNormalizer.normalize(&input);
        let nfd = NfdNormalizer.normalize(&input);
        let nfkc = NfkcNormalizer.normalize(&input);
        let nfkd = NfkdNormalizer.normalize(&input);

        assert!(is_borrowed(&nfc), "NFC should borrow {}-byte ASCII", len);
        assert!(is_borrowed(&nfd), "NFD should borrow {}-byte ASCII", len);
        assert!(is_borrowed(&nfkc), "NFKC should borrow {}-byte ASCII", len);
        assert!(is_borrowed(&nfkd), "NFKD should borrow {}-byte ASCII", len);

        assert_eq!(&*nfc, &input);
        assert_all_forms_match_icu(&input);
    }
}

#[test]
fn boundary_sweep_with_trailing_non_ascii_around_64() {
    // For each ASCII prefix length in [59..=65], append a multi-byte char
    // so that the non-ASCII portion crosses the 64-byte mark.
    for ascii_len in 59..=65 {
        // 2-byte trailing
        let input_2 = format!("{}\u{00E9}", ascii_bytes(ascii_len));
        assert_all_forms_match_icu(&input_2);

        // 3-byte trailing (CJK U+4E00)
        let input_3 = format!("{}\u{4E00}", ascii_bytes(ascii_len));
        assert_all_forms_match_icu(&input_3);

        // 4-byte trailing (emoji U+1F600)
        let input_4 = format!("{}\u{1F600}", ascii_bytes(ascii_len));
        assert_all_forms_match_icu(&input_4);
    }
}

#[test]
fn boundary_sweep_with_trailing_non_ascii_around_128() {
    for ascii_len in 123..=129 {
        let input_2 = format!("{}\u{00E9}", ascii_bytes(ascii_len));
        assert_all_forms_match_icu(&input_2);

        let input_3 = format!("{}\u{4E00}", ascii_bytes(ascii_len));
        assert_all_forms_match_icu(&input_3);

        let input_4 = format!("{}\u{1F600}", ascii_bytes(ascii_len));
        assert_all_forms_match_icu(&input_4);
    }
}

// ============================================================================
// Additional boundary stress: multi-byte sequences at exact 64 boundaries
// ============================================================================

#[test]
fn exact_64_bytes_ending_with_multibyte() {
    // 62 ASCII + 2-byte char = exactly 64 bytes
    let input_2 = format!("{}\u{00E9}", ascii_bytes(62));
    assert_eq!(input_2.len(), 64);
    assert_all_forms_match_icu(&input_2);

    // 61 ASCII + 3-byte char = exactly 64 bytes
    let input_3 = format!("{}\u{4E00}", ascii_bytes(61));
    assert_eq!(input_3.len(), 64);
    assert_all_forms_match_icu(&input_3);

    // 60 ASCII + 4-byte char = exactly 64 bytes
    let input_4 = format!("{}\u{1F600}", ascii_bytes(60));
    assert_eq!(input_4.len(), 64);
    assert_all_forms_match_icu(&input_4);
}

#[test]
fn exact_128_bytes_ending_with_multibyte() {
    // 126 ASCII + 2-byte char = exactly 128 bytes
    let input_2 = format!("{}\u{00E9}", ascii_bytes(126));
    assert_eq!(input_2.len(), 128);
    assert_all_forms_match_icu(&input_2);

    // 125 ASCII + 3-byte char = exactly 128 bytes
    let input_3 = format!("{}\u{4E00}", ascii_bytes(125));
    assert_eq!(input_3.len(), 128);
    assert_all_forms_match_icu(&input_3);

    // 124 ASCII + 4-byte char = exactly 128 bytes
    let input_4 = format!("{}\u{1F600}", ascii_bytes(124));
    assert_eq!(input_4.len(), 128);
    assert_all_forms_match_icu(&input_4);
}

// ============================================================================
// Combining sequences precisely at boundaries
// ============================================================================

#[test]
fn combining_sequence_split_exactly_at_64() {
    // Place a base char at byte 63 with combining mark at bytes 64-65.
    // The SIMD scanner processes bytes 0-63 in chunk 1, then the combining
    // mark lands in chunk 2, but logically it belongs with the base char.
    let prefix = ascii_bytes(63);
    // 'e' (1 byte) at position 63, combining acute U+0301 (2 bytes) at 64-65
    let input = format!("{}e\u{0301}", prefix);
    assert_eq!(input.len(), 66, "63 + 1 + 2 = 66 bytes");
    assert_all_forms_match_icu(&input);

    // NFC should compose e + combining acute -> e-acute (U+00E9)
    let nfc = NfcNormalizer.normalize(&input);
    let expected_nfc = format!("{}\u{00E9}", prefix);
    assert_eq!(
        &*nfc, &expected_nfc,
        "NFC should compose e + acute to e-acute"
    );
}

#[test]
fn combining_sequence_split_exactly_at_128() {
    let prefix = ascii_bytes(127);
    // 'e' (1 byte) at position 127, combining acute U+0301 (2 bytes) at 128-129
    let input = format!("{}e\u{0301}", prefix);
    assert_eq!(input.len(), 130, "127 + 1 + 2 = 130 bytes");
    assert_all_forms_match_icu(&input);

    let nfc = NfcNormalizer.normalize(&input);
    let expected_nfc = format!("{}\u{00E9}", prefix);
    assert_eq!(&*nfc, &expected_nfc);
}

#[test]
fn multiple_combining_marks_crossing_boundary() {
    // 61 ASCII + 'a' (byte 61) + combining diaeresis U+0308 (2 bytes, 62-63) +
    // combining macron U+0304 (2 bytes, 64-65) + combining dot below U+0323 (2 bytes, 66-67)
    let prefix = ascii_bytes(61);
    let input = format!("{}a\u{0308}\u{0304}\u{0323}", prefix);
    assert_eq!(input.len(), 68, "61 + 1 + 2 + 2 + 2 = 68 bytes");
    assert_all_forms_match_icu(&input);
}

// ============================================================================
// Stress: non-ASCII at every position near the boundary
// ============================================================================

#[test]
fn non_ascii_at_every_position_near_64() {
    // For positions 60..=67, place a decomposing character (U+00C0, A-grave,
    // decomposes in NFD to A + combining grave).
    for pos in 60..=67 {
        let mut input = ascii_bytes(pos);
        input.push('\u{00C0}');
        // Pad to at least 70 bytes so there is content after the boundary.
        while input.len() < 70 {
            input.push('z');
        }
        assert_all_forms_match_icu(&input);
    }
}

#[test]
fn non_ascii_at_every_position_near_128() {
    for pos in 124..=131 {
        let mut input = ascii_bytes(pos);
        input.push('\u{00C0}');
        while input.len() < 134 {
            input.push('z');
        }
        assert_all_forms_match_icu(&input);
    }
}

// ============================================================================
// Mixed multi-chunk with boundary-crossing sequences
// ============================================================================

#[test]
fn three_chunks_with_boundary_straddling() {
    // Build a 192+ byte input with multi-byte chars at each 64-byte boundary.
    let mut input = ascii_bytes(62);
    input.push('\u{00C5}'); // A-ring at byte 62-63 (straddles chunk 1 boundary)
    // Pad to byte 126
    while input.len() < 126 {
        input.push('x');
    }
    input.push('\u{4E00}'); // CJK at bytes 126-128 (straddles chunk 2 boundary)
    // Pad to byte 189
    while input.len() < 189 {
        input.push('y');
    }
    input.push('\u{1F600}'); // emoji at bytes 189-192 (straddles chunk 3 boundary)
    input.push_str("tail");
    assert_all_forms_match_icu(&input);
}

// ============================================================================
// Cow::Borrowed verification for already-normalized non-ASCII inputs
// ============================================================================

#[test]
fn nfc_stable_input_at_boundary_returns_borrowed() {
    // Build a 64-byte input that is already in NFC.
    // 61 ASCII + U+4E00 (3 bytes, CJK, NFC-stable) = 64 bytes.
    let input = format!("{}\u{4E00}", ascii_bytes(61));
    assert_eq!(input.len(), 64);

    let nfc = NfcNormalizer.normalize(&input);
    assert!(
        is_borrowed(&nfc),
        "NFC should return Borrowed for NFC-stable 64-byte input"
    );

    // Also check NFKC (CJK has no compat decomposition, so also stable).
    let nfkc = NfkcNormalizer.normalize(&input);
    assert!(
        is_borrowed(&nfkc),
        "NFKC should return Borrowed for NFKC-stable 64-byte input"
    );
}

#[test]
fn nfd_stable_input_at_boundary_returns_borrowed() {
    // NFD-stable: already decomposed input at the 64-byte boundary.
    // 60 ASCII + 'A' (1 byte) + U+030A combining ring above (2 bytes) = 63 bytes.
    // This is the NFD form of A-ring. Pad to 64 bytes.
    let input = format!("{}A\u{030A}z", ascii_bytes(60));
    assert_eq!(input.len(), 64);

    let nfd = NfdNormalizer.normalize(&input);
    assert!(
        is_borrowed(&nfd),
        "NFD should return Borrowed for already-decomposed 64-byte input"
    );
}

// ============================================================================
// Large input: varied pattern ensuring many boundary crossings
// ============================================================================

#[test]
fn large_varied_pattern_1mb() {
    // Build a pattern that ensures multi-byte characters cross chunk boundaries
    // in different positions across repetitions.
    // Pattern: 37 ASCII + U+00E9 (2) + 13 ASCII + U+4E00 (3) + U+1F600 (4) + U+0301 (2)
    // = 37 + 2 + 13 + 3 + 4 + 2 = 61 bytes.  Since 61 is coprime with 64,
    // the multi-byte chars will fall at different offsets in each SIMD chunk.
    let ascii_37 = ascii_bytes(37);
    let ascii_13 = ascii_bytes(13);
    let pattern = format!("{}\u{00E9}{}\u{4E00}\u{1F600}\u{0301}", ascii_37, ascii_13);
    assert_eq!(pattern.len(), 61);

    let target_size = 1024 * 1024;
    let repeats = target_size / pattern.len() + 1;
    let large_input: String = pattern.repeat(repeats);

    let our_nfc = NfcNormalizer.normalize(&large_input).into_owned();
    let our_nfd = NfdNormalizer.normalize(&large_input).into_owned();
    let our_nfkc = NfkcNormalizer.normalize(&large_input).into_owned();
    let our_nfkd = NfkdNormalizer.normalize(&large_input).into_owned();

    let ref_nfc = icu_nfc(&large_input);
    let ref_nfd = icu_nfd(&large_input);
    let ref_nfkc = icu_nfkc(&large_input);
    let ref_nfkd = icu_nfkd(&large_input);

    assert_eq!(&our_nfc, &ref_nfc, "NFC mismatch on large varied input");
    assert_eq!(&our_nfd, &ref_nfd, "NFD mismatch on large varied input");
    assert_eq!(&our_nfkc, &ref_nfkc, "NFKC mismatch on large varied input");
    assert_eq!(&our_nfkd, &ref_nfkd, "NFKD mismatch on large varied input");
}
