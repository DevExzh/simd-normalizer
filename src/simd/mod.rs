//! SIMD-accelerated byte scanning.
//!
//! This module provides the scanner infrastructure that produces per-chunk
//! bitmasks for the normalizer's main loop. The scanner identifies byte
//! positions >= a passthrough bound, flagging them for scalar normalization.
//!
//! Backends:
//! - `scalar` -- always available, no special CPU features
//! - (future) `x86_64::sse42`, `x86_64::avx2`, `x86_64::avx512`
//! - (future) `aarch64::neon`
//! - (future) `wasm32::simd128`

pub(crate) mod prefetch;
pub(crate) mod scanner;
pub(crate) mod scalar;

#[cfg(target_arch = "x86_64")]
pub(crate) mod x86_64;

#[cfg(target_arch = "aarch64")]
pub(crate) mod aarch64;

#[cfg(target_arch = "wasm32")]
pub(crate) mod wasm32;

/// Find the byte offset of the first byte >= `bound` in `bytes`.
///
/// Uses the scalar scanner in 64-byte chunks, then scans the tail
/// byte-by-byte. Returns `bytes.len()` if no such byte exists.
#[inline]
pub(crate) fn find_first_above(bytes: &[u8], bound: u8) -> usize {
    let len = bytes.len();
    let mut offset = 0usize;

    // Process full 64-byte chunks.
    while offset + 64 <= len {
        // SAFETY: offset + 64 <= len, so the pointer is valid for 64 bytes.
        let mask = unsafe { scalar::scan_chunk(bytes.as_ptr().add(offset), bound) };
        if mask != 0 {
            return offset + mask.trailing_zeros() as usize;
        }
        offset += 64;
    }

    // Scan remaining bytes one at a time.
    while offset < len {
        if bytes[offset] >= bound {
            return offset;
        }
        offset += 1;
    }

    len
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_first_above_all_below() {
        let data = b"Hello, world! This is pure ASCII and should return len.";
        assert_eq!(find_first_above(data, 0xC0), data.len());
    }

    #[test]
    fn find_first_above_first_byte() {
        let data = [0xC0u8, 0x00, 0x00, 0x00];
        assert_eq!(find_first_above(&data, 0xC0), 0);
    }

    #[test]
    fn find_first_above_in_second_chunk() {
        let mut data = [0u8; 128];
        data[65] = 0xC0;
        assert_eq!(find_first_above(&data, 0xC0), 65);
    }

    #[test]
    fn find_first_above_in_tail() {
        let mut data = [0u8; 70];
        data[68] = 0xC0;
        assert_eq!(find_first_above(&data, 0xC0), 68);
    }

    #[test]
    fn find_first_above_empty() {
        assert_eq!(find_first_above(&[], 0xC0), 0);
    }

    #[test]
    fn find_first_above_exact_chunk_boundary() {
        let mut data = [0u8; 64];
        data[63] = 0xC0;
        assert_eq!(find_first_above(&data, 0xC0), 63);
    }

    #[test]
    fn find_first_above_multi_chunk_utf8() {
        let s = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\u{00E9}";
        let bytes = s.as_bytes();
        assert_eq!(find_first_above(bytes, 0xC0), 64);
    }
}
