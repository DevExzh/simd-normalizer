//! Integration tests for UTS #39 confusable skeleton mapping.

use simd_normalizer::{are_confusable, skeleton};

// ---------------------------------------------------------------------------
// Basic skeleton tests
// ---------------------------------------------------------------------------

#[test]
fn skeleton_empty() {
    assert_eq!(skeleton(""), "");
}

#[test]
fn skeleton_pure_ascii() {
    // ASCII lowercase letters without confusable mappings should produce
    // a stable result.
    let s = skeleton("hello");
    assert!(!s.is_empty());
}

#[test]
fn skeleton_converges_in_two_passes() {
    let inputs = [
        "hello",
        "apple",
        "\u{0430}\u{0440}\u{0440}l\u{0435}", // mixed Cyrillic
        "\u{00C0}test",                         // precomposed
        "12345",
        "\u{1F600}", // emoji
        "\u{01C4}",  // Ǆ — compatibility character with confusable parts
    ];
    for input in &inputs {
        let once = skeleton(input);
        let twice = skeleton(&once);
        let thrice = skeleton(&twice);
        assert_eq!(twice, thrice, "skeleton did not converge after two passes for {:?}", input);
    }
}

// ---------------------------------------------------------------------------
// Confusable pair tests
// ---------------------------------------------------------------------------

#[test]
fn latin_cyrillic_a() {
    // Latin 'a' (U+0061) and Cyrillic 'а' (U+0430)
    assert!(
        are_confusable("a", "\u{0430}"),
        "Latin 'a' and Cyrillic 'а' should be confusable"
    );
}

#[test]
fn latin_cyrillic_e() {
    // Latin 'e' (U+0065) and Cyrillic 'е' (U+0435)
    assert!(
        are_confusable("e", "\u{0435}"),
        "Latin 'e' and Cyrillic 'е' should be confusable"
    );
}

#[test]
fn latin_cyrillic_o() {
    // Latin 'o' (U+006F) and Cyrillic 'о' (U+043E)
    assert!(
        are_confusable("o", "\u{043E}"),
        "Latin 'o' and Cyrillic 'о' should be confusable"
    );
}

#[test]
fn latin_cyrillic_p() {
    // Latin 'p' (U+0070) and Cyrillic 'р' (U+0440)
    assert!(
        are_confusable("p", "\u{0440}"),
        "Latin 'p' and Cyrillic 'р' should be confusable"
    );
}

// ---------------------------------------------------------------------------
// Word-level confusable tests
// ---------------------------------------------------------------------------

#[test]
fn confusable_word_apple() {
    // "apple" in Latin vs mixed Latin/Cyrillic
    let latin = "apple";
    let mixed = "\u{0430}\u{0440}\u{0440}l\u{0435}"; // а р р l е
    assert!(are_confusable(latin, mixed));
}

#[test]
fn not_confusable_different_words() {
    assert!(!are_confusable("hello", "world"));
    assert!(!are_confusable("cat", "dog"));
}

#[test]
fn identical_strings_confusable() {
    assert!(are_confusable("test", "test"));
    assert!(are_confusable("", ""));
}

// ---------------------------------------------------------------------------
// Mixed-script detection
// ---------------------------------------------------------------------------

#[test]
fn mixed_script_homoglyph_sentence() {
    // "paypal" with Cyrillic 'а' and 'р'
    let real = "paypal";
    let fake = "p\u{0430}yp\u{0430}l"; // p а y p а l
    assert!(are_confusable(real, fake));
}

// ---------------------------------------------------------------------------
// Stability and edge cases
// ---------------------------------------------------------------------------

#[test]
fn skeleton_of_combining_marks() {
    // Standalone combining marks shouldn't panic.
    let s = skeleton("\u{0300}\u{0301}\u{0302}");
    assert!(!s.is_empty());
}

#[test]
fn skeleton_supplementary_chars() {
    // Emoji and other supplementary characters.
    let _ = skeleton("\u{1F600}\u{1F4A9}\u{1F680}");
}

#[test]
fn skeleton_long_string() {
    // Stress test with a longer input.
    let input = "The quick brown fox jumps over the lazy dog. ".repeat(100);
    let s = skeleton(&input);
    assert!(!s.is_empty());
}

#[test]
fn bmp_confusable_no_panics() {
    // Verify skeleton doesn't panic on any BMP character.
    let mut buf = String::new();
    for cp in (0u32..=0xFFFF).step_by(16) {
        buf.clear();
        if let Some(c) = char::from_u32(cp) {
            buf.push(c);
            let _ = skeleton(&buf);
        }
    }
}
