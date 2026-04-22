//! Unicode case folding examples using simd-normalizer.
//!
//! Demonstrates standard and Turkish locale case folding at both the
//! character and string level, the Cow::Borrowed zero-allocation
//! optimization, and a practical case-insensitive comparison helper.
//!
//! Run with:
//!     cargo run --example case_folding

use std::borrow::Cow;

use simd_normalizer::{CaseFoldMode, casefold, casefold_char};

fn main() {
    println!("=== simd-normalizer: Case Folding Examples ===\n");

    section_standard_folding();
    section_turkish_folding();
    section_char_level();
    section_zero_copy();
    section_case_insensitive_comparison();

    println!("=== All examples completed. ===");
}

// ---------------------------------------------------------------------------
// Section 1: Standard Case Folding (CaseFoldMode::Standard)
// ---------------------------------------------------------------------------

fn section_standard_folding() {
    println!("--- 1. Standard Case Folding (CaseFoldMode::Standard) ---\n");

    let examples: &[(&str, &str)] = &[
        ("Hello World", "simple ASCII mixed case"),
        ("HELLO", "all uppercase ASCII"),
        ("Stra\u{00DF}e", "German Strasse with sharp s (U+00DF)"),
        ("Str\u{00F6}me", "German with o-umlaut (U+00F6)"),
        ("CAF\u{00C9}", "CAFE with precomposed E-acute (U+00C9)"),
        (
            "\u{0391}\u{03B8}\u{03AE}\u{03BD}\u{03B1}",
            "Greek: mixed case",
        ),
    ];

    for &(input, description) in examples {
        let folded = casefold(input, CaseFoldMode::Standard);
        println!("  Input:  {:?}  ({})", input, description);
        println!("  Folded: {:?}\n", &*folded);
    }
}

// ---------------------------------------------------------------------------
// Section 2: Turkish Locale Folding (CaseFoldMode::Turkish)
// ---------------------------------------------------------------------------

fn section_turkish_folding() {
    println!("--- 2. Turkish Locale Folding (CaseFoldMode::Turkish) ---\n");

    println!("  Turkish has special rules for the letter I:");
    println!("    Standard: I (U+0049) -> i (U+0069)");
    println!("    Turkish:  I (U+0049) -> \u{0131} (U+0131, dotless i)");
    println!("    Turkish:  \u{0130} (U+0130, dotted I) -> i (U+0069)\n");

    // Compare standard vs Turkish folding of "Istanbul"
    let input = "Istanbul";
    let standard = casefold(input, CaseFoldMode::Standard);
    let turkish = casefold(input, CaseFoldMode::Turkish);
    println!("  Input:            {:?}", input);
    println!("  Standard folded:  {:?}", &*standard);
    println!("  Turkish folded:   {:?}  (I -> dotless i)\n", &*turkish);

    // Dotted capital I (U+0130) -- specific to Turkish/Azerbaijani.
    // Note: In standard mode, U+0130 has no simple (single-char) fold -- its full
    // fold is "i" + combining dot above (two chars), which simple folding cannot
    // represent.  So simple casefold passes it through unchanged.  Turkish mode
    // maps it to plain "i".
    let dotted_i_input = "\u{0130}stanbul";
    let standard_dotted = casefold(dotted_i_input, CaseFoldMode::Standard);
    let turkish_dotted = casefold(dotted_i_input, CaseFoldMode::Turkish);
    println!(
        "  Input:            {:?}  (starts with dotted capital I, U+0130)",
        dotted_i_input
    );
    println!(
        "  Standard folded:  {:?}  (unchanged -- no simple single-char fold)",
        &*standard_dotted
    );
    println!(
        "  Turkish folded:   {:?}  (dotted I -> i)\n",
        &*turkish_dotted
    );

    // Non-I characters fold identically in both modes
    let other = "Ankara";
    let std_other = casefold(other, CaseFoldMode::Standard);
    let tr_other = casefold(other, CaseFoldMode::Turkish);
    println!("  Input:            {:?}", other);
    println!("  Standard folded:  {:?}", &*std_other);
    println!(
        "  Turkish folded:   {:?}  (same -- no I involved)",
        &*tr_other
    );

    println!();
}

// ---------------------------------------------------------------------------
// Section 3: Character-Level Folding (casefold_char)
// ---------------------------------------------------------------------------

fn section_char_level() {
    println!("--- 3. Character-Level Folding (casefold_char) ---\n");

    let chars: &[(char, &str)] = &[
        ('A', "Latin capital A"),
        ('Z', "Latin capital Z"),
        ('\u{00C9}', "Latin capital E-acute (U+00C9)"),
        ('\u{00D6}', "Latin capital O-umlaut (U+00D6)"),
        ('\u{0391}', "Greek capital Alpha (U+0391)"),
        ('\u{0410}', "Cyrillic capital A (U+0410)"),
        ('\u{00B5}', "Micro sign (U+00B5) -> Greek small mu"),
        ('\u{1E9E}', "Latin capital sharp S (U+1E9E) -> sharp s"),
        ('a', "already lowercase -- unchanged"),
        ('7', "digit -- unchanged"),
    ];

    for &(ch, description) in chars {
        let folded = casefold_char(ch, CaseFoldMode::Standard);
        let changed = if folded != ch { "changed" } else { "unchanged" };
        println!("  {:?} -> {:?}  ({}, {})", ch, folded, description, changed);
    }

    println!("\n  Turkish-specific character folding:");
    let i_upper = 'I';
    println!(
        "    Standard: {:?} -> {:?}",
        i_upper,
        casefold_char(i_upper, CaseFoldMode::Standard)
    );
    println!(
        "    Turkish:  {:?} -> {:?}  (dotless i, U+0131)",
        i_upper,
        casefold_char(i_upper, CaseFoldMode::Turkish)
    );

    let dotted_i = '\u{0130}';
    println!(
        "    Turkish:  {:?} -> {:?}  (dotted capital I -> i)",
        dotted_i,
        casefold_char(dotted_i, CaseFoldMode::Turkish)
    );

    println!();
}

// ---------------------------------------------------------------------------
// Section 4: Cow::Borrowed Zero-Allocation Optimization
// ---------------------------------------------------------------------------

fn section_zero_copy() {
    println!("--- 4. Cow::Borrowed Zero-Allocation Optimization ---\n");

    println!("  When the input is already fully case-folded, casefold() returns");
    println!("  Cow::Borrowed -- a zero-copy reference with no heap allocation.\n");

    let test_cases: &[(&str, &str)] = &[
        ("hello world", "all lowercase ASCII"),
        ("already lowercase", "all lowercase ASCII"),
        ("caf\u{00E9}", "lowercase with precomposed e-acute"),
        ("", "empty string"),
        ("Hello", "has uppercase -- will allocate"),
        ("UPPER", "all uppercase -- will allocate"),
    ];

    for &(input, description) in test_cases {
        let folded = casefold(input, CaseFoldMode::Standard);
        let is_borrowed = matches!(&folded, Cow::Borrowed(_));
        println!(
            "  {:?} ({}) -> Cow::{} {:?}",
            input,
            description,
            if is_borrowed { "Borrowed" } else { "Owned" },
            &*folded
        );
    }

    println!("\n  Tip: In hot loops, Cow::Borrowed avoids allocation entirely.");
    println!("  This matters for large datasets where most strings are already lowercase.");

    println!();
}

// ---------------------------------------------------------------------------
// Section 5: Practical Use Case -- Case-Insensitive String Comparison
// ---------------------------------------------------------------------------

fn section_case_insensitive_comparison() {
    println!("--- 5. Practical Use Case: Case-Insensitive String Comparison ---\n");

    /// Compare two strings case-insensitively using Unicode case folding.
    ///
    /// This is more correct than `.to_lowercase()` comparison because it
    /// follows the Unicode CaseFolding.txt specification (status C+S).
    fn eq_ignore_case(a: &str, b: &str, mode: CaseFoldMode) -> bool {
        let fa = casefold(a, mode);
        let fb = casefold(b, mode);
        *fa == *fb
    }

    let pairs: &[(&str, &str, &str)] = &[
        ("Hello", "hello", "ASCII case difference"),
        ("CAF\u{00C9}", "caf\u{00E9}", "accented characters"),
        ("\u{00DF}", "\u{1E9E}", "sharp s: small vs capital"),
        ("\u{00B5}", "\u{03BC}", "micro sign vs Greek mu"),
        ("abc", "def", "different strings"),
    ];

    println!("  Standard mode comparisons:\n");
    for &(a, b, description) in pairs {
        let equal = eq_ignore_case(a, b, CaseFoldMode::Standard);
        println!("    {:?} == {:?}  =>  {}  ({})", a, b, equal, description);
    }

    println!("\n  Turkish mode -- the I problem:\n");
    let a = "Istanbul";
    let b = "\u{0131}stanbul"; // with dotless i
    let c = "istanbul"; // with regular i

    println!("    Comparing {:?} with {:?}:", a, b);
    println!(
        "      Standard: {}  Turkish: {}",
        eq_ignore_case(a, b, CaseFoldMode::Standard),
        eq_ignore_case(a, b, CaseFoldMode::Turkish),
    );
    println!("    Comparing {:?} with {:?}:", a, c);
    println!(
        "      Standard: {}  Turkish: {}",
        eq_ignore_case(a, c, CaseFoldMode::Standard),
        eq_ignore_case(a, c, CaseFoldMode::Turkish),
    );
    println!("\n    In Turkish locale, 'I' folds to dotless i (U+0131), not 'i'.");
    println!("    So \"Istanbul\" matches \"\u{0131}stanbul\" in Turkish mode,");
    println!("    but matches \"istanbul\" only in standard mode.");

    println!();
}
