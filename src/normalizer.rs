//! Single-pass SIMD-guided normalizer implementations (NFC, NFD, NFKC, NFKD).
//!
//! The core loop scans 64-byte chunks via SIMD to identify passthrough regions
//! (all bytes below a form-dependent bound), copying them directly.  Non-passthrough
//! bytes trigger scalar decode + decompose + CCC sort + optional recomposition.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use crate::ccc::{CccBuffer, CharAndCcc};
use crate::compose;
use crate::decompose::{self, DecompForm};
use crate::quick_check;
use crate::simd;
use crate::utf8;

// ---------------------------------------------------------------------------
// Form enum
// ---------------------------------------------------------------------------

/// Unicode normalization form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
    /// Canonical Decomposition, followed by Canonical Composition (NFC).
    Nfc,
    /// Canonical Decomposition (NFD).
    Nfd,
    /// Compatibility Decomposition, followed by Canonical Composition (NFKC).
    Nfkc,
    /// Compatibility Decomposition (NFKD).
    Nfkd,
}

impl Form {
    /// The SIMD passthrough byte bound for this form.
    ///
    /// Any byte below this value is guaranteed to not require normalization
    /// processing: it is either ASCII or a continuation byte of a character
    /// that does not need decomposition.
    ///
    /// - NFD/NFKD: 0xC0  (first byte of U+00C0, which decomposes)
    /// - NFC/NFKC: 0xC0  (same: characters >= U+00C0 may need processing)
    #[inline]
    fn passthrough_bound(self) -> u8 {
        match self {
            Form::Nfc | Form::Nfkc => 0xC0,
            Form::Nfd | Form::Nfkd => 0xC0,
        }
    }

    /// Whether this form applies canonical composition after decomposition.
    #[inline]
    fn composes(self) -> bool {
        matches!(self, Form::Nfc | Form::Nfkc)
    }

    /// Which decomposition form to use.
    #[inline]
    fn decomp_form(self) -> DecompForm {
        match self {
            Form::Nfc | Form::Nfd => DecompForm::Canonical,
            Form::Nfkc | Form::Nfkd => DecompForm::Compatible,
        }
    }
}

// ---------------------------------------------------------------------------
// NormState -- accumulation state for a starter + its combining marks
// ---------------------------------------------------------------------------

struct NormState {
    /// The current starter character (CCC == 0) being accumulated.
    current_starter: Option<char>,
    /// Combining marks (CCC > 0) following the current starter, not yet sorted.
    ccc_buf: CccBuffer,
}

impl NormState {
    fn new() -> Self {
        NormState {
            current_starter: None,
            ccc_buf: CccBuffer::new(),
        }
    }

    /// Flush the current accumulation (starter + combining marks) to `out`.
    ///
    /// If `composes` is true, applies canonical composition.
    fn flush(&mut self, out: &mut String, composes: bool) {
        let starter = match self.current_starter.take() {
            Some(s) => s,
            None => {
                // No starter -- flush any orphan combining marks (leading combiners).
                if !self.ccc_buf.is_empty() {
                    let sorted: Vec<CharAndCcc> = self.ccc_buf.sort_and_drain().collect();
                    for entry in &sorted {
                        out.push(entry.ch);
                    }
                }
                return;
            }
        };

        if self.ccc_buf.is_empty() {
            // Starter with no combining marks -- just emit it.
            out.push(starter);
            return;
        }

        // Collect and sort combining marks by CCC.
        let sorted: Vec<CharAndCcc> = self.ccc_buf.sort_and_drain().collect();

        if composes {
            let (composed, remaining) =
                compose::compose_combining_sequence(starter, &sorted);
            out.push(composed);
            for ch in &remaining {
                out.push(*ch);
            }
        } else {
            // Decomposition only: emit starter + sorted marks.
            out.push(starter);
            for entry in &sorted {
                out.push(entry.ch);
            }
        }
    }

    /// Process a single character (after decomposition) into the accumulation state.
    ///
    /// Characters with CCC == 0 are starters. When a new starter arrives, the
    /// previous accumulation is flushed. In composition mode, starter-to-starter
    /// composition is attempted first (required for Hangul jamo L+V, LV+T).
    fn feed_entry(&mut self, ch: char, ccc: u8, out: &mut String, composes: bool) {
        if ccc == 0 {
            // New starter.
            if composes && self.ccc_buf.is_empty() {
                // No intervening combining marks -- try starter-to-starter composition.
                if let Some(prev) = self.current_starter
                    && let Some(composed) = compose::compose(prev, ch)
                {
                    self.current_starter = Some(composed);
                    return;
                }
            }
            // Either not composing, has intervening marks, or composition failed.
            self.flush(out, composes);
            self.current_starter = Some(ch);
        } else {
            // Combining mark: add to buffer.
            self.ccc_buf.push(ch, ccc);
        }
    }
}

// ---------------------------------------------------------------------------
// process_char -- decompose a char and feed entries to NormState
// ---------------------------------------------------------------------------

/// Decompose a character and feed each resulting entry into the accumulation state.
fn process_char(ch: char, state: &mut NormState, out: &mut String, form: Form) {
    let mut decomp_buf = CccBuffer::new();
    decompose::decompose(ch, &mut decomp_buf, form.decomp_form());

    // Walk the decomposed entries.
    let entries: Vec<CharAndCcc> = decomp_buf.as_slice().to_vec();
    decomp_buf.clear();

    for entry in &entries {
        state.feed_entry(entry.ch, entry.ccc, out, form.composes());
    }
}

// ---------------------------------------------------------------------------
// normalize_scalar -- fallback for short inputs
// ---------------------------------------------------------------------------

/// Normalize a string using pure scalar processing (no SIMD).
fn normalize_scalar<'a>(input: &'a str, form: Form) -> Cow<'a, str> {
    if input.is_empty() {
        return Cow::Borrowed(input);
    }

    let mut out = String::with_capacity(input.len());
    let mut state = NormState::new();

    for ch in input.chars() {
        process_char(ch, &mut state, &mut out, form);
    }

    // Flush any remaining state.
    state.flush(&mut out, form.composes());

    if out == input {
        Cow::Borrowed(input)
    } else {
        Cow::Owned(out)
    }
}

// ---------------------------------------------------------------------------
// normalize_impl -- main SIMD-accelerated loop
// ---------------------------------------------------------------------------

/// Core normalization function.
///
/// Uses SIMD scanning for inputs >= 64 bytes, with scalar fallback for shorter
/// inputs and tails. Returns `Cow::Borrowed` if the input was already normalized.
fn normalize_impl<'a>(input: &'a str, form: Form) -> Cow<'a, str> {
    let bytes = input.as_bytes();
    let len = bytes.len();

    // Short inputs: use scalar path directly.
    if len < 64 {
        return normalize_scalar(input, form);
    }

    let bound = form.passthrough_bound();
    let composes = form.composes();

    // Track whether we have transitioned to Owned mode.
    let mut owned: Option<String> = None;
    // `last_written`: the byte offset up to which we have either:
    //   - confirmed passthrough (Borrowed mode), or
    //   - copied/processed into `owned` (Owned mode).
    let mut last_written: usize = 0;
    let mut state = NormState::new();

    let mut pos: usize = 0;

    // SIMD chunk loop.
    while pos + 64 <= len {
        let chunk_start = pos;
        let ptr = bytes.as_ptr();

        // SAFETY: pos + 64 <= len, so ptr.add(pos) is valid for 64 bytes.
        let mask = unsafe { simd::scan_chunk(ptr.add(pos), bound) };

        if mask == 0 {
            // All passthrough: no bytes >= bound in this chunk.
            pos += 64;
            continue;
        }

        // There are non-passthrough bytes in this chunk.
        // If we haven't switched to Owned yet, do so now.
        if owned.is_none() {
            let mut s = String::with_capacity(len + len / 4);
            // Copy all validated passthrough bytes up to chunk_start.
            s.push_str(&input[last_written..chunk_start]);
            owned = Some(s);
            last_written = chunk_start;
        }

        // Walk set bits in the mask.
        let mut chunk_mask = mask;
        while chunk_mask != 0 {
            let bit_pos = chunk_mask.trailing_zeros() as usize;
            chunk_mask &= chunk_mask.wrapping_sub(1); // clear lowest set bit

            let byte_pos = chunk_start + bit_pos;

            // Skip if we already processed past this position (multi-byte char from previous bit).
            if byte_pos < last_written {
                continue;
            }

            // Skip continuation bytes -- they belong to a char whose leading byte
            // was already processed.
            if utf8::is_continuation_byte(bytes[byte_pos]) {
                continue;
            }

            // Copy any passthrough bytes between last_written and this position.
            // Flush NormState first: it may hold a buffered starter that must
            // appear *before* the passthrough run in the output.
            //
            // In composition mode, keep the last passthrough character as a
            // potential starter for the following combining mark. Passthrough
            // bytes are guaranteed to be ASCII (< 0xC0) and thus single-byte
            // starters with CCC 0.
            if byte_pos > last_written
                && let Some(ref mut out) = owned
            {
                state.flush(out, composes);
                let pass = &input[last_written..byte_pos];
                let n = pass.len();
                if composes {
                    if n > 1 {
                        out.push_str(&pass[..n - 1]);
                    }
                    let last_ch = pass.as_bytes()[n - 1] as char;
                    state.feed_entry(last_ch, 0, out, true);
                } else {
                    out.push_str(pass);
                }
            }

            // Decode the character at this position.
            let (ch, width) = utf8::decode_char_at(bytes, byte_pos);
            last_written = byte_pos + width;

            // Process through decomposition + accumulation.
            if let Some(ref mut out) = owned {
                process_char(ch, &mut state, out, form);
            }
        }

        pos += 64;
    }

    // Scalar tail: remaining bytes after the last full chunk.
    if pos < len {
        // Check if the tail has any non-passthrough bytes.
        let tail_has_work = bytes[pos..].iter().any(|&b| b >= bound);

        if tail_has_work {
            if owned.is_none() {
                let mut s = String::with_capacity(len + len / 4);
                s.push_str(&input[last_written..pos]);
                owned = Some(s);
                last_written = pos;
            }

            // Process remaining bytes character-by-character.
            let mut tail_pos = pos;
            while tail_pos < len {
                if tail_pos < last_written {
                    tail_pos += 1;
                    continue;
                }

                if utf8::is_continuation_byte(bytes[tail_pos]) {
                    tail_pos += 1;
                    continue;
                }

                // Copy passthrough bytes before this char.
                // Flush NormState first to preserve correct ordering.
                // In composition mode, keep the last passthrough char as starter.
                if tail_pos > last_written
                    && let Some(ref mut out) = owned
                {
                    state.flush(out, composes);
                    let pass = &input[last_written..tail_pos];
                    let n = pass.len();
                    if composes {
                        if n > 1 {
                            out.push_str(&pass[..n - 1]);
                        }
                        let last_ch = pass.as_bytes()[n - 1] as char;
                        state.feed_entry(last_ch, 0, out, true);
                    } else {
                        out.push_str(pass);
                    }
                }

                let (ch, width) = utf8::decode_char_at(bytes, tail_pos);
                last_written = tail_pos + width;

                if let Some(ref mut out) = owned {
                    process_char(ch, &mut state, out, form);
                }

                tail_pos += width;
            }
        }
    }

    // If we never switched to Owned mode, the input is already normalized.
    match owned {
        None => Cow::Borrowed(input),
        Some(mut out) => {
            // Flush any remaining state.
            state.flush(&mut out, composes);

            // Copy any trailing passthrough bytes.
            if last_written < len {
                out.push_str(&input[last_written..len]);
            }

            if out == input {
                Cow::Borrowed(input)
            } else {
                Cow::Owned(out)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Public normalizer types
// ---------------------------------------------------------------------------

/// NFC normalizer: Canonical Decomposition, followed by Canonical Composition.
pub struct NfcNormalizer;

/// NFD normalizer: Canonical Decomposition.
pub struct NfdNormalizer;

/// NFKC normalizer: Compatibility Decomposition, followed by Canonical Composition.
pub struct NfkcNormalizer;

/// NFKD normalizer: Compatibility Decomposition.
pub struct NfkdNormalizer;

impl Default for NfcNormalizer {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for NfdNormalizer {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for NfkcNormalizer {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for NfkdNormalizer {
    fn default() -> Self {
        Self::new()
    }
}

impl NfcNormalizer {
    /// Create a new NFC normalizer.
    pub fn new() -> Self {
        NfcNormalizer
    }

    /// Run the NFC quick-check algorithm on `input`.
    pub fn quick_check(&self, input: &str) -> crate::quick_check::IsNormalized {
        quick_check::quick_check_nfc(input)
    }

    /// Normalize the input string to NFC form.
    ///
    /// Returns `Cow::Borrowed` if the input is already in NFC.
    pub fn normalize<'a>(&self, input: &'a str) -> Cow<'a, str> {
        normalize_impl(input, Form::Nfc)
    }

    /// Normalize the input string to NFC form, appending to `out`.
    ///
    /// Returns `true` if the input was already normalized (nothing was modified).
    pub fn normalize_to(&self, input: &str, out: &mut String) -> bool {
        let result = normalize_impl(input, Form::Nfc);
        let already_normalized = matches!(&result, Cow::Borrowed(_));
        out.push_str(&result);
        already_normalized
    }

    /// Check if the input is already in NFC form.
    pub fn is_normalized(&self, input: &str) -> bool {
        quick_check::is_normalized_nfc(input)
    }
}

impl NfdNormalizer {
    /// Create a new NFD normalizer.
    pub fn new() -> Self {
        NfdNormalizer
    }

    /// Run the NFD quick-check algorithm on `input`.
    pub fn quick_check(&self, input: &str) -> crate::quick_check::IsNormalized {
        quick_check::quick_check_nfd(input)
    }

    /// Normalize the input string to NFD form.
    ///
    /// Returns `Cow::Borrowed` if the input is already in NFD.
    pub fn normalize<'a>(&self, input: &'a str) -> Cow<'a, str> {
        normalize_impl(input, Form::Nfd)
    }

    /// Normalize the input string to NFD form, appending to `out`.
    ///
    /// Returns `true` if the input was already normalized (nothing was modified).
    pub fn normalize_to(&self, input: &str, out: &mut String) -> bool {
        let result = normalize_impl(input, Form::Nfd);
        let already_normalized = matches!(&result, Cow::Borrowed(_));
        out.push_str(&result);
        already_normalized
    }

    /// Check if the input is already in NFD form.
    pub fn is_normalized(&self, input: &str) -> bool {
        quick_check::is_normalized_nfd(input)
    }
}

impl NfkcNormalizer {
    /// Create a new NFKC normalizer.
    pub fn new() -> Self {
        NfkcNormalizer
    }

    /// Run the NFKC quick-check algorithm on `input`.
    pub fn quick_check(&self, input: &str) -> crate::quick_check::IsNormalized {
        quick_check::quick_check_nfkc(input)
    }

    /// Normalize the input string to NFKC form.
    ///
    /// Returns `Cow::Borrowed` if the input is already in NFKC.
    pub fn normalize<'a>(&self, input: &'a str) -> Cow<'a, str> {
        normalize_impl(input, Form::Nfkc)
    }

    /// Normalize the input string to NFKC form, appending to `out`.
    ///
    /// Returns `true` if the input was already normalized (nothing was modified).
    pub fn normalize_to(&self, input: &str, out: &mut String) -> bool {
        let result = normalize_impl(input, Form::Nfkc);
        let already_normalized = matches!(&result, Cow::Borrowed(_));
        out.push_str(&result);
        already_normalized
    }

    /// Check if the input is already in NFKC form.
    pub fn is_normalized(&self, input: &str) -> bool {
        quick_check::is_normalized_nfkc(input)
    }
}

impl NfkdNormalizer {
    /// Create a new NFKD normalizer.
    pub fn new() -> Self {
        NfkdNormalizer
    }

    /// Run the NFKD quick-check algorithm on `input`.
    pub fn quick_check(&self, input: &str) -> crate::quick_check::IsNormalized {
        quick_check::quick_check_nfkd(input)
    }

    /// Normalize the input string to NFKD form.
    ///
    /// Returns `Cow::Borrowed` if the input is already in NFKD.
    pub fn normalize<'a>(&self, input: &'a str) -> Cow<'a, str> {
        normalize_impl(input, Form::Nfkd)
    }

    /// Normalize the input string to NFKD form, appending to `out`.
    ///
    /// Returns `true` if the input was already normalized (nothing was modified).
    pub fn normalize_to(&self, input: &str, out: &mut String) -> bool {
        let result = normalize_impl(input, Form::Nfkd);
        let already_normalized = matches!(&result, Cow::Borrowed(_));
        out.push_str(&result);
        already_normalized
    }

    /// Check if the input is already in NFKD form.
    pub fn is_normalized(&self, input: &str) -> bool {
        quick_check::is_normalized_nfkd(input)
    }
}
