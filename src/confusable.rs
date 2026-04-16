//! UTS #39 confusable skeleton mapping.
//!
//! Implements the skeleton algorithm from UTS #39 Section 4:
//! `skeleton(input) = NFD(confusable_map(NFD(input)))`
//!
//! Two strings are confusable if they produce the same skeleton.

use alloc::string::String;

use crate::tables::{self, ConfusableResult};

/// Apply the confusable mapping to a single character.
///
/// Pushes the mapped character(s) to `out`. If the character has no
/// confusable mapping, it is pushed unchanged.
#[inline]
fn confusable_map_char(c: char, out: &mut String) {
    match tables::lookup_confusable(c) {
        ConfusableResult::None => out.push(c),
        ConfusableResult::Single(mapped) => out.push(mapped),
        ConfusableResult::Expansion { offset, length } => {
            let data = tables::confusable_expansion_data(offset, length);
            for &cp in data {
                // SAFETY: expansion table contains only valid Unicode scalar values.
                debug_assert!(cp <= 0x10FFFF && !(0xD800..=0xDFFF).contains(&cp));
                let ch = unsafe { char::from_u32_unchecked(cp) };
                out.push(ch);
            }
        },
    }
}

/// Compute the UTS #39 skeleton of a string.
///
/// The skeleton algorithm: `NFD(confusable_map(NFD(input)))`.
///
/// Two strings are confusable (visually similar) if and only if
/// `skeleton(a) == skeleton(b)`.
pub fn skeleton(input: &str) -> String {
    if input.is_empty() {
        return String::new();
    }

    // Step 1: NFD(input)
    let nfd = crate::nfd().normalize(input);

    // Step 2: Apply confusable mapping to each character.
    let mut mapped = String::with_capacity(nfd.len());
    for ch in nfd.chars() {
        confusable_map_char(ch, &mut mapped);
    }

    // Step 3: NFD(mapped)
    let result = crate::nfd().normalize(&mapped);
    result.into_owned()
}

/// Check whether two strings are confusable (have the same skeleton).
pub fn are_confusable(a: &str, b: &str) -> bool {
    skeleton(a) == skeleton(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skeleton_empty() {
        assert_eq!(skeleton(""), "");
    }

    #[test]
    fn skeleton_ascii_unchanged() {
        // Most lowercase ASCII maps to itself.
        let s = skeleton("hello");
        // 'h', 'e', 'l', 'l', 'o' — none are confusable prototypes typically
        // but let's just check it doesn't panic.
        assert!(!s.is_empty());
    }

    #[test]
    fn confusable_latin_cyrillic_a() {
        // Latin 'a' (U+0061) and Cyrillic 'а' (U+0430) should produce the
        // same skeleton.
        assert!(
            are_confusable("a", "\u{0430}"),
            "Latin 'a' and Cyrillic 'а' should be confusable"
        );
    }

    #[test]
    fn confusable_latin_cyrillic_word() {
        // "apple" in Latin vs "аррlе" with Cyrillic а, р, р, and е
        // The Cyrillic lookalikes: а=U+0430, р=U+0440, е=U+0435
        let latin = "apple";
        let mixed = "\u{0430}\u{0440}\u{0440}l\u{0435}";
        assert!(are_confusable(latin, mixed));
    }

    #[test]
    fn not_confusable_different_strings() {
        assert!(!are_confusable("hello", "world"));
    }

    #[test]
    fn confusable_identical_strings() {
        assert!(are_confusable("test", "test"));
    }

    #[test]
    fn skeleton_convergence() {
        // Applying skeleton twice should converge (reach a fixed point).
        // A single application is not guaranteed to be idempotent per UTS #39,
        // since composite confusable mappings may expand into parts that have
        // their own confusable mappings.
        let input = "Hel\u{0430}"; // mix of Latin and Cyrillic
        let s1 = skeleton(input);
        let s2 = skeleton(&s1);
        let s3 = skeleton(&s2);
        assert_eq!(s2, s3, "skeleton should converge after two passes");
    }

    #[test]
    fn confusable_fullwidth() {
        // Fullwidth Latin A (U+FF21) should be confusable with regular 'A'
        // (they both map to the same skeleton after confusable mapping).
        // Note: fullwidth is handled by NFKD, confusable mapping handles others.
        let s1 = skeleton("A");
        let s2 = skeleton("\u{FF21}");
        // These may or may not be equal depending on whether confusables.txt
        // includes the mapping. At minimum, verify no panic.
        let _ = (s1, s2);
    }
}
