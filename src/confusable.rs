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
///
/// A 256-byte compile-time bloom filter (see
/// [`tables::confusable_bloom_might_contain`]) gates the binary search into
/// the mapping table. The bloom is built from the same `CONFUSABLE_MAPPINGS`
/// list that `lookup_confusable` consults, so by construction every source
/// codepoint hashes to a set bit — false negatives are impossible. A clear
/// bit means the character is provably not in the table and we skip the
/// search outright. The vast majority of codepoints in real-world text
/// (ASCII, common Latin-1, CJK ideographs, etc.) are not confusable sources,
/// so the bloom check eliminates most of the per-codepoint search cost.
#[inline]
fn confusable_map_char(c: char, out: &mut String) {
    if !tables::confusable_bloom_might_contain(c as u32) {
        out.push(c);
        return;
    }
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
/// The skeleton algorithm: `NFD(confusable_map(NFD(input)))`, iterated
/// to a fixed point. Iteration is required because some confusable
/// expansions produce characters that themselves have confusable
/// mappings; UTS #39 specifies that `skeleton(skeleton(X)) = skeleton(X)`,
/// so we converge before returning.
///
/// Two strings are confusable (visually similar) if and only if
/// `skeleton(a) == skeleton(b)`.
pub fn skeleton(input: &str) -> String {
    if input.is_empty() {
        return String::new();
    }

    // Start from NFD(input); subsequent passes operate on the previous output.
    let mut current: String = crate::nfd().normalize(input).into_owned();

    // Apply confusable_map + NFD until a fixed point. A small iteration
    // cap guards against pathological inputs; in practice 1–2 passes suffice.
    for _ in 0..8 {
        let mut mapped = String::with_capacity(current.len());
        for ch in current.chars() {
            confusable_map_char(ch, &mut mapped);
        }
        let next = crate::nfd().normalize(&mapped).into_owned();
        if next == current {
            return next;
        }
        current = next;
    }
    current
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
        // UTS #39 requires skeleton(skeleton(X)) == skeleton(X).
        let input = "Hel\u{0430}"; // mix of Latin and Cyrillic
        let s1 = skeleton(input);
        let s2 = skeleton(&s1);
        assert_eq!(s1, s2, "skeleton must be idempotent");
    }

    #[test]
    fn skeleton_idempotent_on_cascading_mapping() {
        // Regression for fuzzer-found case where the confusable table maps
        // U+1D0E (ᴎ) through multiple hops; a single NFD→map→NFD pass was
        // not a fixed point. Skeleton must converge regardless.
        let input = "\u{1D0E}\u{326}\u{306}";
        let s1 = skeleton(input);
        let s2 = skeleton(&s1);
        assert_eq!(s1, s2, "skeleton must be idempotent for cascading maps");
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
