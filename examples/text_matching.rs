//! Text matching examples using simd-normalizer's matching pipeline.
//!
//! Demonstrates fused NFKC + CaseFold + Confusable Skeleton matching,
//! `normalize_for_matching()` for pre-processing, `MatchingOptions` with
//! standard and Turkish modes, and UTF-16 output for interop scenarios.
//!
//! Run with:
//!     cargo run --example text_matching

use simd_normalizer::matching::{
    matches_normalized, normalize_for_matching, normalize_for_matching_utf16, MatchingOptions,
};
use simd_normalizer::CaseFoldMode;

fn main() {
    println!("=== simd-normalizer: Text Matching Examples ===\n");

    section_fused_comparison();
    section_confusable_detection();
    section_compatibility_matching();
    section_normalize_for_indexing();
    section_turkish_mode();
    section_utf16_interop();
    section_search_scenario();
    section_username_comparison();

    println!("=== All text matching examples completed. ===");
}

// ---------------------------------------------------------------------------
// Section 1: matches_normalized() -- fused comparison
// ---------------------------------------------------------------------------

fn section_fused_comparison() {
    println!("--- 1. Fused Comparison with matches_normalized() ---\n");
    println!("  matches_normalized() combines NFKC + case folding + confusable");
    println!("  skeleton mapping in a single comparison step.\n");

    let opts = MatchingOptions::default();

    // Simple case folding
    let result = matches_normalized("File", "file", &opts);
    println!("  matches_normalized(\"File\", \"file\")    = {}", result);
    assert!(result);

    let result = matches_normalized("HELLO", "hello", &opts);
    println!("  matches_normalized(\"HELLO\", \"hello\")  = {}", result);
    assert!(result);

    // Mixed case
    let result = matches_normalized("CaFe", "cafe", &opts);
    println!("  matches_normalized(\"CaFe\", \"cafe\")    = {}", result);
    assert!(result);

    // Non-matching strings
    let result = matches_normalized("hello", "world", &opts);
    println!("  matches_normalized(\"hello\", \"world\")  = {}", result);
    assert!(!result);

    println!();
}

// ---------------------------------------------------------------------------
// Section 2: Confusable character detection
// ---------------------------------------------------------------------------

fn section_confusable_detection() {
    println!("--- 2. Confusable Character Detection ---\n");
    println!("  The matching pipeline detects visually confusable characters");
    println!("  (UTS #39 skeleton) so that spoofing attempts are caught.\n");

    let opts = MatchingOptions::default();

    // Latin 'a' (U+0061) vs Cyrillic 'a' (U+0430)
    let result = matches_normalized("a", "\u{0430}", &opts);
    println!("  Latin 'a' (U+0061) vs Cyrillic '\u{0430}' (U+0430): match = {}", result);
    assert!(result);

    // Mixed-script spoofing: "apple" with Cyrillic lookalikes
    // Cyrillic: a=U+0430, p=U+0440, e=U+0435
    let latin = "apple";
    let spoofed = "\u{0430}\u{0440}\u{0440}l\u{0435}";
    let result = matches_normalized(latin, spoofed, &opts);
    println!(
        "  Latin \"apple\" vs Cyrillic-mixed \"{}\": match = {}",
        spoofed, result
    );
    assert!(result);

    // Identical strings always match (fast path)
    let result = matches_normalized("test", "test", &opts);
    println!("  Identical strings \"test\" vs \"test\":        match = {}", result);
    assert!(result);

    println!();
}

// ---------------------------------------------------------------------------
// Section 3: NFKC compatibility matching
// ---------------------------------------------------------------------------

fn section_compatibility_matching() {
    println!("--- 3. NFKC Compatibility Matching ---\n");
    println!("  NFKC normalization unifies compatibility equivalents such as");
    println!("  fullwidth characters and superscript digits.\n");

    let opts = MatchingOptions::default();

    // Fullwidth 'A' (U+FF21) vs standard 'a'
    let result = matches_normalized("\u{FF21}", "a", &opts);
    println!(
        "  Fullwidth 'A' (U+FF21) vs 'a': match = {}",
        result
    );
    assert!(result);

    // Fullwidth string vs ASCII
    let fullwidth = "\u{FF28}\u{FF45}\u{FF4C}\u{FF4C}\u{FF4F}"; // Fullwidth "Hello"
    let result = matches_normalized(fullwidth, "hello", &opts);
    println!(
        "  Fullwidth \"{}\" vs \"hello\": match = {}",
        fullwidth, result
    );
    assert!(result);

    // Superscript '2' (U+00B2) vs plain '2'
    let result = matches_normalized("\u{00B2}", "2", &opts);
    println!("  Superscript '2' (U+00B2) vs '2': match = {}", result);
    assert!(result);

    println!();
}

// ---------------------------------------------------------------------------
// Section 4: normalize_for_matching() -- pre-processing for storage/indexing
// ---------------------------------------------------------------------------

fn section_normalize_for_indexing() {
    println!("--- 4. Pre-Processing with normalize_for_matching() ---\n");
    println!("  normalize_for_matching() returns a canonical matching form suitable");
    println!("  for storage in search indexes or database columns.\n");

    let opts = MatchingOptions::default();

    // Note: the matching pipeline unifies case, compatibility forms, and
    // confusables, but it preserves combining marks.  "Cafe" and "café" will
    // have different matching keys because the accent is retained.
    let inputs = [
        "Cafe",
        "cafe",
        "CAFE",
        "caf\u{00E9}",         // precomposed e-acute
        "cafe\u{0301}",        // e + combining acute
    ];

    println!("  Case variants normalize to the same form (accents are preserved):");
    for input in &inputs {
        let normalized = normalize_for_matching(input, &opts);
        println!("    {:?} -> {:?}", input, normalized);
    }

    // Demonstrate idempotence
    println!();
    let once = normalize_for_matching("File", &opts);
    let twice = normalize_for_matching(&once, &opts);
    println!("  Idempotence check:");
    println!("    normalize_for_matching(\"File\")      = {:?}", once);
    println!("    normalize_for_matching(result)       = {:?}", twice);
    println!("    Idempotent: {}", once == twice);
    assert_eq!(once, twice);

    println!();
}

// ---------------------------------------------------------------------------
// Section 5: Turkish mode
// ---------------------------------------------------------------------------

fn section_turkish_mode() {
    println!("--- 5. Turkish Mode (MatchingOptions) ---\n");
    println!("  Turkish/Azerbaijani locales have special case folding rules:");
    println!("    Standard: I -> i       Turkish: I -> \\u{{0131}} (dotless i)");
    println!("    Standard: i -> i       Turkish: \\u{{0130}} (dotted I) -> i\n");

    let standard = MatchingOptions::default();
    let turkish = MatchingOptions {
        case_fold: CaseFoldMode::Turkish,
    };

    // In standard mode: "I" folds to "i"
    let std_norm = normalize_for_matching("Istanbul", &standard);
    println!("  Standard mode:");
    println!("    normalize_for_matching(\"Istanbul\") = {:?}", std_norm);

    // In Turkish mode: "I" folds to dotless-i (U+0131), but the confusable
    // skeleton step may map it back to "i" (since ı and i are confusable).
    // The final result can look the same as standard mode in some cases.
    let tr_norm = normalize_for_matching("Istanbul", &turkish);
    println!("  Turkish mode:");
    println!("    normalize_for_matching(\"Istanbul\") = {:?}", tr_norm);

    // Turkish dotted-I (U+0130) folds to 'i' in Turkish mode
    let dotted_i = "\u{0130}stanbul"; // capital dotted I
    let tr_dotted = normalize_for_matching(dotted_i, &turkish);
    println!(
        "    normalize_for_matching(\"\\u{{0130}}stanbul\") = {:?}",
        tr_dotted
    );
    let result = matches_normalized(dotted_i, "istanbul", &turkish);
    println!(
        "    matches_normalized(\"\\u{{0130}}stanbul\", \"istanbul\") = {}",
        result
    );
    assert!(result);

    // Show that Turkish dotless-i matches "I" in Turkish mode
    let result = matches_normalized("Istanbul", "\u{0131}stanbul", &turkish);
    println!(
        "    matches_normalized(\"Istanbul\", \"\\u{{0131}}stanbul\") = {} (Turkish)",
        result
    );
    assert!(result);

    println!();
}

// ---------------------------------------------------------------------------
// Section 6: UTF-16 output for interop
// ---------------------------------------------------------------------------

fn section_utf16_interop() {
    println!("--- 6. UTF-16 Output for Interop ---\n");
    println!("  normalize_for_matching_utf16() produces UTF-16 code units,");
    println!("  useful for interop with Windows APIs, Java, .NET, or databases");
    println!("  that store text as UTF-16.\n");

    let opts = MatchingOptions::default();

    // Basic example
    let utf16 = normalize_for_matching_utf16("Hello", &opts);
    let utf8 = normalize_for_matching("Hello", &opts);
    println!("  Input: \"Hello\"");
    println!("    UTF-8 matching form:  {:?}", utf8);
    println!("    UTF-16 code units:    {:?}", utf16);

    // Round-trip check
    let decoded = String::from_utf16(&utf16).expect("valid UTF-16");
    println!("    Round-trip to String:  {:?}", decoded);
    assert_eq!(decoded, utf8);

    // Supplementary character (emoji) -- requires surrogate pair in UTF-16
    println!();
    let emoji = "\u{1F600}"; // grinning face
    let utf16_emoji = normalize_for_matching_utf16(emoji, &opts);
    println!("  Input: \"{}\" (U+1F600, grinning face)", emoji);
    println!("    UTF-16 code units: {:?}", utf16_emoji);
    println!(
        "    Code unit count: {} (surrogate pair for supplementary char)",
        utf16_emoji.len()
    );

    // Accented text
    println!();
    let accented = "Caf\u{00E9}";
    let utf16_acc = normalize_for_matching_utf16(accented, &opts);
    let utf8_acc = normalize_for_matching(accented, &opts);
    println!("  Input: {:?}", accented);
    println!("    UTF-8 matching form: {:?}", utf8_acc);
    println!("    UTF-16 code units:   {:?}", utf16_acc);

    println!();
}

// ---------------------------------------------------------------------------
// Section 7: Practical search scenario
// ---------------------------------------------------------------------------

fn section_search_scenario() {
    println!("--- 7. Practical Scenario: Search Indexing ---\n");
    println!("  Normalize documents at index time, then normalize queries at search");
    println!("  time. Matching happens via simple string equality on the index.\n");

    let opts = MatchingOptions::default();

    // Simulate indexing: normalize document titles for storage
    let documents = [
        "Resume",
        "r\u{00E9}sum\u{00E9}",  // resume with accents
        "RESUME",
        "R\u{00C9}SUM\u{00C9}",  // uppercase accented
    ];

    println!("  Indexing documents:");
    let index: Vec<(String, &str)> = documents
        .iter()
        .map(|doc| (normalize_for_matching(doc, &opts), *doc))
        .collect();

    for (normalized, original) in &index {
        println!("    {:?} -> index key {:?}", original, normalized);
    }

    // Simulate a search query
    let query = "resume";
    let query_key = normalize_for_matching(query, &opts);
    println!("\n  Search query: {:?} -> key {:?}", query, query_key);

    let hits: Vec<&&str> = index
        .iter()
        .filter(|(key, _)| key == &query_key)
        .map(|(_, original)| original)
        .collect();
    println!("  Matching documents: {:?}", hits);

    println!();
}

// ---------------------------------------------------------------------------
// Section 8: Username comparison
// ---------------------------------------------------------------------------

fn section_username_comparison() {
    println!("--- 8. Practical Scenario: Username Anti-Spoofing ---\n");
    println!("  Detect attempts to register confusable usernames by comparing");
    println!("  the matching-normalized forms of existing and proposed names.\n");

    let opts = MatchingOptions::default();

    let existing_user = "admin";
    let existing_key = normalize_for_matching(existing_user, &opts);

    let attempts = [
        "Admin",                             // case variant
        "ADMIN",                             // all caps
        "\u{0430}dmin",                      // Cyrillic 'a' (U+0430)
        "\u{FF21}dmin",                      // Fullwidth 'A' (U+FF21)
        "adm\u{0131}n",                      // Turkish dotless-i
        "administrator",                     // different name (should not match)
    ];

    println!("  Existing user: {:?} (key: {:?})\n", existing_user, existing_key);

    for attempt in &attempts {
        let attempt_key = normalize_for_matching(attempt, &opts);
        let blocked = attempt_key == existing_key;
        println!(
            "    Attempt {:?} -> key {:?} => {}",
            attempt,
            attempt_key,
            if blocked { "BLOCKED (confusable)" } else { "allowed" }
        );
    }

    println!();
}
