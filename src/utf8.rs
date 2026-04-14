//! UTF-8 decode/encode helpers used by the scanner and normalizer.

/// Returns the number of bytes in a UTF-8 character given the leading byte.
///
/// Returns 0 for continuation bytes (0x80..=0xBF) and invalid leading bytes
/// (0xC0, 0xC1, 0xF5..=0xFF).  Callers that operate on known-valid UTF-8
/// never see a 0 return.
#[inline]
pub(crate) fn utf8_char_width(first_byte: u8) -> usize {
    const WIDTHS: [u8; 16] = [1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 2, 2, 3, 4];
    WIDTHS[(first_byte >> 4) as usize] as usize
}

/// Returns `true` if the byte is a UTF-8 continuation byte (`10xxxxxx`).
#[inline]
pub(crate) fn is_continuation_byte(b: u8) -> bool {
    (b & 0xC0) == 0x80
}

/// Decode one UTF-8 character from `bytes` starting at `pos`.
///
/// Returns `(char, byte_length)`.  The input must be valid UTF-8.
///
/// # Panics
///
/// Panics in debug mode if `pos` is out of bounds or points to a continuation byte.
#[inline]
pub(crate) fn decode_char_at(bytes: &[u8], pos: usize) -> (char, usize) {
    let b0 = bytes[pos];
    let width = utf8_char_width(b0);
    debug_assert!(width > 0, "decode_char_at called on continuation byte");
    let cp = match width {
        1 => b0 as u32,
        2 => ((b0 as u32 & 0x1F) << 6) | (bytes[pos + 1] as u32 & 0x3F),
        3 => {
            ((b0 as u32 & 0x0F) << 12)
                | ((bytes[pos + 1] as u32 & 0x3F) << 6)
                | (bytes[pos + 2] as u32 & 0x3F)
        }
        4 => {
            ((b0 as u32 & 0x07) << 18)
                | ((bytes[pos + 1] as u32 & 0x3F) << 12)
                | ((bytes[pos + 2] as u32 & 0x3F) << 6)
                | (bytes[pos + 3] as u32 & 0x3F)
        }
        _ => unreachable!(),
    };
    // Safety: input comes from valid &str, so cp is a valid Unicode scalar value.
    (unsafe { char::from_u32_unchecked(cp) }, width)
}

/// Encode a Unicode scalar value as UTF-8 into `buf`.
///
/// Returns the number of bytes written (1--4).
#[allow(dead_code)]
#[inline]
pub(crate) fn encode_char(c: char, buf: &mut [u8; 4]) -> usize {
    let cp = c as u32;
    if cp < 0x80 {
        buf[0] = cp as u8;
        1
    } else if cp < 0x800 {
        buf[0] = 0xC0 | (cp >> 6) as u8;
        buf[1] = 0x80 | (cp & 0x3F) as u8;
        2
    } else if cp < 0x10000 {
        buf[0] = 0xE0 | (cp >> 12) as u8;
        buf[1] = 0x80 | ((cp >> 6) & 0x3F) as u8;
        buf[2] = 0x80 | (cp & 0x3F) as u8;
        3
    } else {
        buf[0] = 0xF0 | (cp >> 18) as u8;
        buf[1] = 0x80 | ((cp >> 12) & 0x3F) as u8;
        buf[2] = 0x80 | ((cp >> 6) & 0x3F) as u8;
        buf[3] = 0x80 | (cp & 0x3F) as u8;
        4
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── utf8_char_width ──────────────────────────────────────────

    #[test]
    fn width_ascii() {
        for b in 0x00u8..=0x7F {
            assert_eq!(utf8_char_width(b), 1, "byte {:#04x}", b);
        }
    }

    #[test]
    fn width_two_byte() {
        for b in 0xC2u8..=0xDF {
            assert_eq!(utf8_char_width(b), 2, "byte {:#04x}", b);
        }
    }

    #[test]
    fn width_three_byte() {
        for b in 0xE0u8..=0xEF {
            assert_eq!(utf8_char_width(b), 3, "byte {:#04x}", b);
        }
    }

    #[test]
    fn width_four_byte() {
        for b in 0xF0u8..=0xF4 {
            assert_eq!(utf8_char_width(b), 4, "byte {:#04x}", b);
        }
    }

    // ── decode_char_at ───────────────────────────────────────────

    #[test]
    fn decode_ascii() {
        let s = "Hello";
        let bytes = s.as_bytes();
        let (ch, len) = decode_char_at(bytes, 0);
        assert_eq!(ch, 'H');
        assert_eq!(len, 1);
    }

    #[test]
    fn decode_two_byte_char() {
        let s = "\u{00E9}";
        let bytes = s.as_bytes();
        let (ch, len) = decode_char_at(bytes, 0);
        assert_eq!(ch, '\u{00E9}');
        assert_eq!(len, 2);
    }

    #[test]
    fn decode_three_byte_char() {
        let s = "\u{4E16}";
        let bytes = s.as_bytes();
        let (ch, len) = decode_char_at(bytes, 0);
        assert_eq!(ch, '\u{4E16}');
        assert_eq!(len, 3);
    }

    #[test]
    fn decode_four_byte_char() {
        let s = "\u{1F600}";
        let bytes = s.as_bytes();
        let (ch, len) = decode_char_at(bytes, 0);
        assert_eq!(ch, '\u{1F600}');
        assert_eq!(len, 4);
    }

    #[test]
    fn decode_at_offset() {
        let s = "A\u{00E9}B";
        let bytes = s.as_bytes();
        let (ch, len) = decode_char_at(bytes, 1);
        assert_eq!(ch, '\u{00E9}');
        assert_eq!(len, 2);
        let (ch2, len2) = decode_char_at(bytes, 3);
        assert_eq!(ch2, 'B');
        assert_eq!(len2, 1);
    }

    // ── encode_char ──────────────────────────────────────────────

    #[test]
    fn encode_ascii_char() {
        let mut buf = [0u8; 4];
        let len = encode_char('A', &mut buf);
        assert_eq!(len, 1);
        assert_eq!(&buf[..len], b"A");
    }

    #[test]
    fn encode_two_byte_char() {
        let mut buf = [0u8; 4];
        let len = encode_char('\u{00E9}', &mut buf);
        assert_eq!(len, 2);
        assert_eq!(&buf[..len], "\u{00E9}".as_bytes());
    }

    #[test]
    fn encode_three_byte_char() {
        let mut buf = [0u8; 4];
        let len = encode_char('\u{4E16}', &mut buf);
        assert_eq!(len, 3);
        assert_eq!(&buf[..len], "\u{4E16}".as_bytes());
    }

    #[test]
    fn encode_four_byte_char() {
        let mut buf = [0u8; 4];
        let len = encode_char('\u{1F600}', &mut buf);
        assert_eq!(len, 4);
        assert_eq!(&buf[..len], "\u{1F600}".as_bytes());
    }

    #[test]
    fn encode_roundtrip() {
        for &c in &['A', '\u{00E9}', '\u{4E16}', '\u{1F600}', '\u{0300}'] {
            let mut buf = [0u8; 4];
            let len = encode_char(c, &mut buf);
            let (decoded, dec_len) = decode_char_at(&buf, 0);
            assert_eq!(decoded, c);
            assert_eq!(dec_len, len);
        }
    }

    // ── is_continuation_byte ─────────────────────────────────────

    #[test]
    fn continuation_byte_true() {
        for b in 0x80u8..=0xBF {
            assert!(
                is_continuation_byte(b),
                "byte {:#04x} should be continuation",
                b
            );
        }
    }

    #[test]
    fn continuation_byte_false_ascii() {
        for b in 0x00u8..=0x7F {
            assert!(
                !is_continuation_byte(b),
                "byte {:#04x} should not be continuation",
                b
            );
        }
    }

    #[test]
    fn continuation_byte_false_leading() {
        for b in 0xC0u8..=0xFF {
            assert!(
                !is_continuation_byte(b),
                "byte {:#04x} should not be continuation",
                b
            );
        }
    }
}
