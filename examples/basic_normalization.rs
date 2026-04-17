//! Basic Unicode normalization examples using simd-normalizer.
//!
//! Demonstrates all four normalization forms (NFC, NFD, NFKC, NFKD),
//! both the trait-based API and the constructor-based API, zero-copy
//! detection, quick_check, is_normalized, and normalize_to with buffer reuse.
//!
//! Run with:
//!     cargo run --example basic_normalization

use std::borrow::Cow;

use simd_normalizer::UnicodeNormalization;

fn main() {
    println!("=== simd-normalizer: Basic Normalization Examples ===\n");

    section_four_forms();
    section_trait_api();
    section_constructor_api();
    section_zero_copy();
    section_quick_check();
    section_normalize_to();
    section_real_world_examples();

    println!("=== All examples completed. ===");
}

// ---------------------------------------------------------------------------
// Section 1: The Four Normalization Forms
// ---------------------------------------------------------------------------

fn section_four_forms() {
    println!("--- 1. The Four Normalization Forms ---\n");

    // e followed by combining acute accent (U+0301)
    let decomposed = "e\u{0301}";
    // precomposed e-acute (U+00E9)
    let precomposed = "\u{00E9}";

    println!("Input (decomposed): {:?}  (e + \\u{{0301}} combining acute)", decomposed);
    println!("Input (precomposed): {:?}  (\\u{{00E9}} e-acute)\n", precomposed);

    // NFC: Canonical Decomposition, then Canonical Composition
    // Composes decomposed sequences into precomposed characters where possible.
    let nfc_result = decomposed.nfc();
    println!("NFC  of {:?} = {:?}", decomposed, &*nfc_result);
    println!("  NFC composes: e + \\u{{0301}} -> \\u{{00E9}} (precomposed e-acute)");

    // NFD: Canonical Decomposition
    // Decomposes precomposed characters into their canonical decomposition.
    let nfd_result = precomposed.nfd();
    println!("NFD  of {:?} = {:?}", precomposed, &*nfd_result);
    println!("  NFD decomposes: \\u{{00E9}} -> e + \\u{{0301}}");

    // NFKC: Compatibility Decomposition, then Canonical Composition
    // Like NFC but also replaces compatibility characters.
    let nfkc_result = "\u{FB01}".nfkc(); // fi ligature
    println!("NFKC of {:?} (fi ligature) = {:?}", "\u{FB01}", &*nfkc_result);
    println!("  NFKC decomposes compatibility characters: fi ligature -> \"fi\"");

    // NFKD: Compatibility Decomposition
    // Like NFD but also replaces compatibility characters.
    let nfkd_result = "\u{FB01}".nfkd();
    println!("NFKD of {:?} (fi ligature) = {:?}", "\u{FB01}", &*nfkd_result);
    println!("  NFKD decomposes compatibility characters without recomposing");

    println!();
}

// ---------------------------------------------------------------------------
// Section 2: Trait API (UnicodeNormalization for &str)
// ---------------------------------------------------------------------------

fn section_trait_api() {
    println!("--- 2. Trait API (UnicodeNormalization) ---\n");

    let input = "caf\u{00E9}";
    println!("Input: {:?}", input);

    let nfc: Cow<'_, str> = input.nfc();
    let nfd: Cow<'_, str> = input.nfd();
    let nfkc: Cow<'_, str> = input.nfkc();
    let nfkd: Cow<'_, str> = input.nfkd();

    println!("  .nfc()  = {:?}", &*nfc);
    println!("  .nfd()  = {:?}", &*nfd);
    println!("  .nfkc() = {:?}", &*nfkc);
    println!("  .nfkd() = {:?}", &*nfkd);

    // is_* checks via the trait
    println!("  .is_nfc()  = {}", input.is_nfc());
    println!("  .is_nfd()  = {}", input.is_nfd());
    println!("  .is_nfkc() = {}", input.is_nfkc());
    println!("  .is_nfkd() = {}", input.is_nfkd());

    println!();
}

// ---------------------------------------------------------------------------
// Section 3: Constructor API
// ---------------------------------------------------------------------------

fn section_constructor_api() {
    println!("--- 3. Constructor API (simd_normalizer::nfc(), etc.) ---\n");

    let normalizer = simd_normalizer::nfc();

    let input = "e\u{0301}"; // decomposed e-acute
    println!("Input: {:?}  (e + combining acute)", input);

    let result = normalizer.normalize(input);
    println!("  normalizer.normalize() = {:?}", &*result);

    let is_norm = normalizer.is_normalized(input);
    println!("  normalizer.is_normalized() = {}", is_norm);

    let qc = normalizer.quick_check(input);
    println!("  normalizer.quick_check()   = {:?}", qc);

    println!();
}

// ---------------------------------------------------------------------------
// Section 4: Zero-copy Cow::Borrowed detection
// ---------------------------------------------------------------------------

fn section_zero_copy() {
    println!("--- 4. Zero-Copy (Cow::Borrowed) Detection ---\n");

    let normalizer = simd_normalizer::nfc();

    // Already NFC -- the library returns a Cow::Borrowed, avoiding allocation.
    let already_nfc = "caf\u{00E9}";
    let result = normalizer.normalize(already_nfc);
    let borrowed = matches!(&result, Cow::Borrowed(_));
    println!(
        "Input: {:?} (already NFC)",
        already_nfc
    );
    println!("  Result is Cow::Borrowed (zero alloc): {}", borrowed);

    // Not NFC -- normalization is needed, so Cow::Owned is returned.
    let not_nfc = "e\u{0301}";
    let result2 = normalizer.normalize(not_nfc);
    let borrowed2 = matches!(&result2, Cow::Borrowed(_));
    println!(
        "Input: {:?} (not NFC, needs normalization)",
        not_nfc
    );
    println!("  Result is Cow::Borrowed (zero alloc): {}", borrowed2);

    // Pure ASCII -- always already normalized in every form.
    let ascii = "Hello, world!";
    let result3 = normalizer.normalize(ascii);
    let borrowed3 = matches!(&result3, Cow::Borrowed(_));
    println!(
        "Input: {:?} (pure ASCII)",
        ascii
    );
    println!("  Result is Cow::Borrowed (zero alloc): {}", borrowed3);

    println!();
}

// ---------------------------------------------------------------------------
// Section 5: quick_check() and is_normalized()
// ---------------------------------------------------------------------------

fn section_quick_check() {
    println!("--- 5. quick_check() and is_normalized() ---\n");

    let nfc = simd_normalizer::nfc();
    let nfd = simd_normalizer::nfd();

    let inputs = [
        ("Hello", "pure ASCII"),
        ("\u{00E9}", "precomposed e-acute (U+00E9)"),
        ("e\u{0301}", "e + combining acute (decomposed)"),
        ("\u{AC00}", "Hangul syllable GA (U+AC00)"),
    ];

    println!("  quick_check returns IsNormalized::Yes, No, or Maybe.");
    println!("  is_normalized resolves Maybe by performing full normalization.\n");

    for (input, desc) in &inputs {
        let nfc_qc = nfc.quick_check(input);
        let nfc_is = nfc.is_normalized(input);
        let nfd_qc = nfd.quick_check(input);
        let nfd_is = nfd.is_normalized(input);
        println!("  {:?} ({})", input, desc);
        println!("    NFC  quick_check={:?}, is_normalized={}", nfc_qc, nfc_is);
        println!("    NFD  quick_check={:?}, is_normalized={}", nfd_qc, nfd_is);
    }

    println!();
}

// ---------------------------------------------------------------------------
// Section 6: normalize_to() with pre-allocated buffer
// ---------------------------------------------------------------------------

fn section_normalize_to() {
    println!("--- 6. normalize_to() with Pre-Allocated Buffer ---\n");

    let normalizer = simd_normalizer::nfc();

    // Pre-allocate a buffer and reuse it across multiple normalizations.
    let mut buf = String::with_capacity(256);

    let inputs = [
        "e\u{0301}",      // decomposed e-acute
        "caf\u{00E9}",    // already NFC
        "\u{1100}\u{1161}", // Hangul jamo L+V -> syllable
    ];

    for input in &inputs {
        buf.clear();
        let was_already = normalizer.normalize_to(input, &mut buf);
        println!("  Input: {:?}", input);
        println!("    Output: {:?}", buf);
        println!("    Was already normalized: {}", was_already);
    }

    println!("\n  Buffer reuse avoids repeated heap allocations in tight loops.");
    println!();
}

// ---------------------------------------------------------------------------
// Section 7: Real Unicode examples
// ---------------------------------------------------------------------------

fn section_real_world_examples() {
    println!("--- 7. Real-World Unicode Examples ---\n");

    // -- E-acute composition/decomposition --
    println!("  [E-acute composition]");
    let decomposed = "e\u{0301}";
    let composed = decomposed.nfc();
    println!("    NFD  \"e\\u{{0301}}\" -> NFC {:?}", &*composed);
    let round_trip = composed.nfd();
    println!("    NFC  {:?} -> NFD {:?}", &*composed, &*round_trip);

    // -- Hangul syllables --
    println!("\n  [Hangul syllables]");
    // A precomposed Hangul syllable decomposes to jamo in NFD.
    let hangul = "\u{AC00}"; // GA = L(U+1100) + V(U+1161)
    let nfd_hangul = hangul.nfd();
    println!(
        "    Hangul GA (U+AC00) NFD = {:?}  (jamo L \\u{{1100}} + V \\u{{1161}})",
        &*nfd_hangul
    );
    // Jamo compose back into a syllable in NFC.
    let jamo = "\u{1100}\u{1161}";
    let nfc_jamo = jamo.nfc();
    println!(
        "    Jamo \\u{{1100}}\\u{{1161}} NFC = {:?}  (syllable GA U+AC00)",
        &*nfc_jamo
    );
    // Hangul with trailing consonant
    let hangul_lvt = "\u{D55C}"; // HAN = L + A + N
    let nfd_lvt = hangul_lvt.nfd();
    println!(
        "    Hangul HAN (U+D55C) NFD = {:?}  (L + A + N jamo)",
        &*nfd_lvt
    );

    // -- Fullwidth characters --
    println!("\n  [Fullwidth characters (compatibility)]");
    let fullwidth = "\u{FF21}\u{FF22}\u{FF23}"; // fullwidth ABC
    println!("    Fullwidth: {:?}", fullwidth);
    let nfkc_fw = fullwidth.nfkc();
    println!("    NFKC: {:?}  (normalized to standard ASCII)", &*nfkc_fw);
    let nfc_fw = fullwidth.nfc();
    println!("    NFC:  {:?}  (canonical normalization does NOT change compatibility chars)", &*nfc_fw);

    // -- fi ligature (compatibility decomposition) --
    println!("\n  [fi ligature (compatibility decomposition)]");
    let fi = "\u{FB01}";
    println!("    Input: {:?}  (U+FB01 LATIN SMALL LIGATURE FI)", fi);
    println!("    NFC:  {:?}  (unchanged -- canonical only)", &*fi.nfc());
    println!("    NFKC: {:?}  (decomposed to f + i)", &*fi.nfkc());
    println!("    NFKD: {:?}  (decomposed to f + i)", &*fi.nfkd());

    // -- Superscript digits --
    println!("\n  [Superscript digits (compatibility decomposition)]");
    // U+00B2 = superscript 2, U+00B3 = superscript 3, U+00B9 = superscript 1
    let superscripts = "\u{00B9}\u{00B2}\u{00B3}";
    println!("    Input: {:?}  (superscript 1, 2, 3)", superscripts);
    println!("    NFC:  {:?}  (unchanged)", &*superscripts.nfc());
    println!("    NFKC: {:?}  (decomposed to plain digits)", &*superscripts.nfkc());
    println!("    NFKD: {:?}  (decomposed to plain digits)", &*superscripts.nfkd());

    // -- Comparison: canonical vs compatibility --
    println!("\n  [Key takeaway]");
    println!("    NFC/NFD  = canonical normalization (preserves meaning)");
    println!("    NFKC/NFKD = compatibility normalization (lossy: strips formatting)");
    println!("    Use NFC for storage, NFKC for search/matching.");

    println!();
}
