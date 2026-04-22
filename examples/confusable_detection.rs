//! Confusable (homoglyph) detection examples using simd-normalizer.
//!
//! Demonstrates how to detect visually similar but distinct Unicode strings
//! using UTS #39 confusable mapping. This is critical for anti-spoofing in
//! domain names, usernames, and other security-sensitive identifiers.
//!
//! Run with:
//!     cargo run --example confusable_detection

use simd_normalizer::{are_confusable, skeleton};

fn main() {
    println!("=== simd-normalizer: Confusable Detection Examples ===\n");

    section_why_confusables_matter();
    section_basic_confusable_check();
    section_skeleton_mapping();
    section_domain_name_spoofing();
    section_username_spoofing();
    section_building_a_lookup_table();

    println!("=== All confusable detection examples completed. ===");
}

// ---------------------------------------------------------------------------
// Section 1: Why Confusable Detection Matters
// ---------------------------------------------------------------------------

fn section_why_confusables_matter() {
    println!("--- 1. Why Confusable Detection Matters ---\n");

    println!("  Many Unicode characters from different scripts look nearly identical:");
    println!("    Latin  'a' (U+0061)  vs  Cyrillic '\u{0430}' (U+0430)");
    println!("    Latin  'p' (U+0070)  vs  Cyrillic '\u{0440}' (U+0440)");
    println!("    Latin  'e' (U+0065)  vs  Cyrillic '\u{0435}' (U+0435)");
    println!("    Latin  'o' (U+006F)  vs  Cyrillic '\u{043E}' (U+043E)");
    println!();
    println!("  An attacker can register \"\u{0430}pple.com\" using Cyrillic '\u{0430}' instead");
    println!("  of Latin 'a'. To a human reader, it looks identical to \"apple.com\".");
    println!();
    println!("  UTS #39 (Unicode Security Mechanisms) defines a 'skeleton' algorithm");
    println!("  that maps visually similar characters to a common form, enabling");
    println!("  detection of these confusable strings.");
    println!();
}

// ---------------------------------------------------------------------------
// Section 2: Basic Confusable Checks
// ---------------------------------------------------------------------------

fn section_basic_confusable_check() {
    println!("--- 2. Basic Confusable Checks (are_confusable) ---\n");

    // Latin 'a' vs Cyrillic 'a'
    let latin_a = "a";
    let cyrillic_a = "\u{0430}";
    let result = are_confusable(latin_a, cyrillic_a);
    println!(
        "  are_confusable(\"a\" [Latin], \"\\u{{0430}}\" [Cyrillic]) = {}",
        result
    );
    println!("    These look identical but are different code points.\n");

    // Latin 'p' vs Cyrillic 'p'
    let latin_p = "p";
    let cyrillic_p = "\u{0440}";
    let result = are_confusable(latin_p, cyrillic_p);
    println!(
        "  are_confusable(\"p\" [Latin], \"\\u{{0440}}\" [Cyrillic]) = {}",
        result
    );

    // Latin 'e' vs Cyrillic 'e'
    let latin_e = "e";
    let cyrillic_e = "\u{0435}";
    let result = are_confusable(latin_e, cyrillic_e);
    println!(
        "  are_confusable(\"e\" [Latin], \"\\u{{0435}}\" [Cyrillic]) = {}",
        result
    );

    // Latin 'o' vs Cyrillic 'o'
    let latin_o = "o";
    let cyrillic_o = "\u{043E}";
    let result = are_confusable(latin_o, cyrillic_o);
    println!(
        "  are_confusable(\"o\" [Latin], \"\\u{{043E}}\" [Cyrillic]) = {}",
        result
    );

    println!();

    // Full word: "apple" vs mixed-script lookalike
    let latin_apple = "apple";
    // Cyrillic a (U+0430), Cyrillic p (U+0440), Cyrillic p (U+0440), Latin l, Cyrillic e (U+0435)
    let spoofed_apple = "\u{0430}\u{0440}\u{0440}l\u{0435}";
    let result = are_confusable(latin_apple, spoofed_apple);
    println!(
        "  are_confusable(\"apple\" [Latin], \"\\u{{0430}}\\u{{0440}}\\u{{0440}}l\\u{{0435}}\" [mixed]) = {}",
        result
    );
    println!("    The spoofed version uses Cyrillic lookalikes for a, p, and e.\n");

    // Strings that are NOT confusable
    let result = are_confusable("hello", "world");
    println!("  are_confusable(\"hello\", \"world\") = {}", result);
    println!("    Completely different strings are not confusable.\n");

    // Identical strings are always confusable (trivially)
    let result = are_confusable("test", "test");
    println!("  are_confusable(\"test\", \"test\") = {}", result);
    println!("    Identical strings always have the same skeleton.");

    println!();
}

// ---------------------------------------------------------------------------
// Section 3: Skeleton Mapping
// ---------------------------------------------------------------------------

fn section_skeleton_mapping() {
    println!("--- 3. Skeleton Mapping (skeleton) ---\n");

    println!("  The skeleton function maps a string to its canonical confusable form.");
    println!("  Two strings are confusable if and only if they share the same skeleton.\n");

    let pairs: &[(&str, &str)] = &[
        ("a", "\u{0430}"),                              // Latin a vs Cyrillic a
        ("apple", "\u{0430}\u{0440}\u{0440}l\u{0435}"), // full word
        ("hello", "hello"),                             // identical
    ];

    for &(left, right) in pairs {
        let skel_left = skeleton(left);
        let skel_right = skeleton(right);
        let confusable = skel_left == skel_right;

        println!("  skeleton({:?})", left);
        println!("    = {:?}", skel_left);
        println!("  skeleton({:?})", right);
        println!("    = {:?}", skel_right);
        println!(
            "  Same skeleton: {} -> confusable: {}\n",
            confusable, confusable
        );
    }

    println!("  The skeleton is an opaque internal form. You should not display it");
    println!("  to users -- only compare skeletons for equality.");
    println!();
}

// ---------------------------------------------------------------------------
// Section 4: Domain Name Spoofing Detection
// ---------------------------------------------------------------------------

fn section_domain_name_spoofing() {
    println!("--- 4. Domain Name Spoofing Detection ---\n");

    println!("  Internationalized Domain Names (IDN) can contain non-ASCII characters.");
    println!("  Attackers exploit this by registering domains that look like trusted ones.\n");

    let legitimate_domains = ["apple.com", "google.com", "paypal.com"];

    // Spoofed variants using Cyrillic lookalikes
    let spoofed_domains = [
        ("\u{0430}pple.com", "apple.com", "Cyrillic 'a' (U+0430)"),
        (
            "g\u{043E}\u{043E}gle.com",
            "google.com",
            "Cyrillic 'o' (U+043E) x2",
        ),
        (
            "p\u{0430}yp\u{0430}l.com",
            "paypal.com",
            "Cyrillic 'a' (U+0430) x2",
        ),
    ];

    for (spoofed, target, description) in &spoofed_domains {
        let is_confusable = are_confusable(spoofed, target);
        println!("  Spoofed: {:?}", spoofed);
        println!("  Target:  {:?}", target);
        println!("  Attack:  {}", description);
        println!("  Confusable: {}", is_confusable);
        println!();
    }

    // Demonstrate a simple domain allow-list check
    println!("  [Practical pattern: domain allow-list check]");
    println!();
    let incoming_domain = "\u{0430}pple.com"; // spoofed with Cyrillic 'a'
    println!("  Incoming domain: {:?}", incoming_domain);

    let incoming_skeleton = skeleton(incoming_domain);
    let mut matched = false;
    for legit in &legitimate_domains {
        let legit_skeleton = skeleton(legit);
        if incoming_skeleton == legit_skeleton {
            println!(
                "  WARNING: {:?} is confusable with trusted domain {:?}",
                incoming_domain, legit
            );
            matched = true;
            break;
        }
    }
    if !matched {
        println!("  OK: No confusable match found in the allow-list.");
    }

    println!();
}

// ---------------------------------------------------------------------------
// Section 5: Username Spoofing Detection
// ---------------------------------------------------------------------------

fn section_username_spoofing() {
    println!("--- 5. Username Spoofing Detection ---\n");

    println!("  Usernames on social platforms, code repositories, and messaging apps");
    println!("  are prime targets for homoglyph attacks. An attacker can impersonate");
    println!("  a trusted user by registering a visually identical name.\n");

    let existing_users = ["admin", "alice", "support"];

    let registration_attempts = [
        ("\u{0430}dmin", "Cyrillic 'a' replacing Latin 'a'"),
        (
            "\u{0430}lic\u{0435}",
            "Cyrillic 'a' and 'e' replacing Latin",
        ),
        ("supp\u{043E}rt", "Cyrillic 'o' replacing Latin 'o'"),
        ("bob", "legitimate new username"),
    ];

    for (new_name, description) in &registration_attempts {
        let new_skeleton = skeleton(new_name);
        let mut conflict = None;

        for existing in &existing_users {
            if skeleton(existing) == new_skeleton {
                conflict = Some(*existing);
                break;
            }
        }

        match conflict {
            Some(existing) => {
                println!(
                    "  REJECTED: {:?} ({}) -- confusable with existing user {:?}",
                    new_name, description, existing
                );
            },
            None => {
                println!(
                    "  ACCEPTED: {:?} ({}) -- no confusable conflict",
                    new_name, description
                );
            },
        }
    }

    println!();
}

// ---------------------------------------------------------------------------
// Section 6: Building a Skeleton Lookup Table
// ---------------------------------------------------------------------------

fn section_building_a_lookup_table() {
    println!("--- 6. Building a Skeleton Lookup Table ---\n");

    println!("  For high-throughput systems, pre-compute skeletons for all known-good");
    println!("  identifiers and store them in a lookup table. Then compare incoming");
    println!("  identifiers by skeleton in O(1) amortized time.\n");

    use std::collections::HashMap;

    // Pre-compute skeletons for trusted identifiers
    let trusted_identifiers = ["apple", "google", "paypal", "admin", "support"];
    let mut skeleton_table: HashMap<String, &str> = HashMap::new();

    for &ident in &trusted_identifiers {
        let skel = skeleton(ident);
        skeleton_table.insert(skel, ident);
    }

    println!("  Trusted identifiers and their skeletons:");
    for &ident in &trusted_identifiers {
        println!("    {:?} -> skeleton {:?}", ident, skeleton(ident));
    }
    println!();

    // Check incoming identifiers against the table
    let incoming = [
        "\u{0430}pple",         // Cyrillic 'a'
        "g\u{043E}\u{043E}gle", // Cyrillic 'o' x2
        "newuser123",           // genuinely new
    ];

    println!("  Checking incoming identifiers:");
    for &input in &incoming {
        let skel = skeleton(input);
        match skeleton_table.get(&skel) {
            Some(trusted) => {
                if *trusted == input {
                    println!("    {:?} -> exact match for {:?}", input, trusted);
                } else {
                    println!(
                        "    {:?} -> SPOOF DETECTED: confusable with {:?}",
                        input, trusted
                    );
                }
            },
            None => {
                println!("    {:?} -> no conflict, safe to register", input);
            },
        }
    }

    println!();
    println!("  [Key takeaway]");
    println!("    Always normalize identifiers through skeleton() before comparison.");
    println!("    Reject or flag registrations whose skeleton matches an existing entry.");
    println!("    Combine with mixed-script detection for defense in depth.");
    println!();
}
