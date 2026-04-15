//! Single-pass SIMD-guided normalizer implementations (NFC, NFD, NFKC, NFKD).
//!
//! The core loop scans 64-byte chunks via SIMD to identify passthrough regions
//! (all bytes below a form-dependent bound), copying them directly.  Non-passthrough
//! bytes trigger scalar decode + decompose + CCC sort + optional recomposition.

use alloc::borrow::Cow;
use alloc::string::String;

use crate::ccc::CccBuffer;
use crate::compose;
use crate::decompose::{self, DecompForm};
use crate::hangul;
use crate::quick_check;
use crate::simd;
use crate::simd::prefetch;
use crate::tables;
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

    /// Estimated output capacity for a given input length.
    #[inline]
    fn estimated_capacity(self, input_len: usize) -> usize {
        match self {
            Form::Nfc | Form::Nfkc => input_len,
            Form::Nfd | Form::Nfkd => input_len + input_len / 2,
        }
    }

    /// Run quick_check for this normalization form.
    #[inline]
    fn quick_check(self, input: &str) -> quick_check::IsNormalized {
        match self {
            Form::Nfc => quick_check::quick_check_nfc(input),
            Form::Nfd => quick_check::quick_check_nfd(input),
            Form::Nfkc => quick_check::quick_check_nfkc(input),
            Form::Nfkd => quick_check::quick_check_nfkd(input),
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
    #[inline]
    fn new() -> Self {
        NormState {
            current_starter: None,
            ccc_buf: CccBuffer::new(),
        }
    }

    /// Flush the current accumulation (starter + combining marks) to `out`.
    ///
    /// If `composes` is true, applies canonical composition.
    #[inline]
    fn flush(&mut self, out: &mut String, composes: bool) {
        let starter = match self.current_starter.take() {
            Some(s) => s,
            None => {
                // No starter -- flush any orphan combining marks (leading combiners).
                if !self.ccc_buf.is_empty() {
                    self.ccc_buf.sort_in_place();
                    for entry in self.ccc_buf.as_slice() {
                        out.push(entry.ch);
                    }
                    self.ccc_buf.clear();
                }
                return;
            }
        };

        if self.ccc_buf.is_empty() {
            // Starter with no combining marks -- just emit it.
            out.push(starter);
            return;
        }

        // Sort combining marks by CCC in place.
        self.ccc_buf.sort_in_place();

        if composes {
            compose::compose_combining_sequence_into(starter, self.ccc_buf.as_slice(), out);
        } else {
            // Decomposition only: emit starter + sorted marks.
            out.push(starter);
            for entry in self.ccc_buf.as_slice() {
                out.push(entry.ch);
            }
        }
        self.ccc_buf.clear();
    }

    /// Process a single character (after decomposition) into the accumulation state.
    ///
    /// Characters with CCC == 0 are starters. When a new starter arrives, the
    /// previous accumulation is flushed. In composition mode, starter-to-starter
    /// composition is attempted first (required for Hangul jamo L+V, LV+T).
    #[inline]
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

    /// NFD-specialized flush: no composition logic.
    #[inline]
    fn flush_nfd(&mut self, out: &mut String) {
        let starter = match self.current_starter.take() {
            Some(s) => s,
            None => {
                if !self.ccc_buf.is_empty() {
                    self.ccc_buf.sort_in_place();
                    for entry in self.ccc_buf.as_slice() {
                        out.push(entry.ch);
                    }
                    self.ccc_buf.clear();
                }
                return;
            }
        };

        // Fast path: single combining mark (most common for precomposed Latin).
        // Skip sort (unnecessary for 1 element) and avoid as_slice/clear overhead.
        if let Some(entry) = self.ccc_buf.take_single_inline() {
            out.push(starter);
            out.push(entry.ch);
            return;
        }

        if self.ccc_buf.is_empty() {
            out.push(starter);
            return;
        }

        // Multiple marks: sort and emit.
        self.ccc_buf.sort_in_place();
        out.push(starter);
        for entry in self.ccc_buf.as_slice() {
            out.push(entry.ch);
        }
        self.ccc_buf.clear();
    }

    /// NFD-specialized feed_entry: no composition checks.
    #[inline]
    fn feed_entry_nfd(&mut self, ch: char, ccc: u8, out: &mut String) {
        if ccc == 0 {
            self.flush_nfd(out);
            self.current_starter = Some(ch);
        } else {
            self.ccc_buf.push(ch, ccc);
        }
    }
}

// ---------------------------------------------------------------------------
// process_char -- decompose a char and feed entries to NormState
// ---------------------------------------------------------------------------

/// Check if a code point is a CJK Unified Ideograph (CCC=0, no decomposition,
/// no composition). These can bypass the entire decompose pipeline.
#[inline(always)]
fn is_cjk_unified(cp: u32) -> bool {
    (0x4E00..=0x9FFF).contains(&cp) || (0x3400..=0x4DBF).contains(&cp)
}

/// Check if a supplementary code point (cp >= 0x10000) is safe (CCC=0, no
/// decomposition in any normalization form). Covers the vast majority of
/// supplementary characters; only narrow exception ranges need trie lookups.
#[inline(always)]
fn is_supp_safe(cp: u32) -> bool {
    if cp >= 0x20000 {
        // Plane 2+: safe except CJK Compatibility Ideographs Supplement
        return !(0x2F800..=0x2FA1F).contains(&cp);
    }
    // Plane 1: core emoji and symbols block (U+1F252-U+1FBEF) is safe.
    // Verified: no decompositions and CCC=0 for all normalization forms.
    (0x1F252..=0x1FBEF).contains(&cp)
}

/// Decompose a character and feed each resulting entry into the accumulation state.
///
/// Uses a single trie lookup with passthrough fast-paths for non-decomposing
/// characters, avoiding the full decomposition pipeline for the common case.
#[inline]
fn process_char(
    ch: char,
    state: &mut NormState,
    out: &mut String,
    form: Form,
    decomp_buf: &mut CccBuffer,
) {
    let cp = ch as u32;

    // Fast path: CJK ideographs never decompose, have CCC=0, and never
    // participate in canonical composition. No trie lookup needed.
    if cp >= 0x3400 && is_cjk_unified(cp) {
        state.flush(out, form.composes());
        state.current_starter = Some(ch);
        return;
    }

    // Hangul syllables: algorithmic decomposition, no trie lookup needed.
    if hangul::is_hangul_syllable(ch) {
        let (l, v, t) = hangul::decompose_hangul(ch);
        state.feed_entry(l, 0, out, form.composes());
        state.feed_entry(v, 0, out, form.composes());
        if let Some(t_char) = t {
            state.feed_entry(t_char, 0, out, form.composes());
        }
        return;
    }

    // Single trie lookup for both passthrough check and decomposition.
    let trie_value = tables::raw_decomp_trie_value(ch, form.decomp_form());

    // Non-decomposing character: extract CCC and feed directly.
    // This covers both starters (CCC=0) and combining marks (CCC>0)
    // that map to themselves, skipping the full decompose pipeline.
    if !tables::has_decomposition(trie_value) {
        let ccc = tables::ccc_from_trie_value(trie_value);
        state.feed_entry(ch, ccc, out, form.composes());
        return;
    }

    // Character has a decomposition: decode from the pre-looked-up trie value.
    decomp_buf.clear();
    decompose::decompose_from_trie_value(ch, trie_value, decomp_buf, form.decomp_form());
    for entry in decomp_buf.as_slice() {
        state.feed_entry(entry.ch, entry.ccc, out, form.composes());
    }
}

/// Process a non-CJK, non-Hangul character using a pre-computed trie value.
///
/// Used by the NFC/NFKC passthrough path in the SIMD loop to avoid a redundant
/// trie lookup (the caller already looked up the trie value to decide whether
/// the character is passthrough).
#[allow(dead_code)]
#[inline(always)]
fn process_from_trie(
    ch: char,
    tv: u32,
    state: &mut NormState,
    out: &mut String,
    form: Form,
    decomp_buf: &mut CccBuffer,
) {
    if !tables::has_decomposition(tv) {
        let ccc = tables::ccc_from_trie_value(tv);
        state.feed_entry(ch, ccc, out, form.composes());
    } else {
        decomp_buf.clear();
        decompose::decompose_from_trie_value(ch, tv, decomp_buf, form.decomp_form());
        for entry in decomp_buf.as_slice() {
            state.feed_entry(entry.ch, entry.ccc, out, form.composes());
        }
    }
}

/// Process a non-CJK, non-Hangul character for NFD/NFKD using a pre-computed
/// trie value. Avoids `DecompResult` enum construction by inlining the expansion
/// path and specializing the common 2-entry case (starter + single combining mark).
#[inline(always)]
fn process_from_trie_nfd(
    ch: char,
    tv: u32,
    state: &mut NormState,
    out: &mut String,
    decomp_form: DecompForm,
) {
    if !tables::has_decomposition(tv) {
        // Non-decomposing character (e.g. combining mark): extract CCC and feed.
        let ccc = tables::ccc_from_trie_value(tv);
        state.feed_entry_nfd(ch, ccc, out);
        return;
    }

    // Fast path: expansion (the vast majority of decomposing BMP characters).
    if let Some(data) = tables::expansion_data_from_trie_value(tv, decomp_form) {
        // Specialize 2-entry expansion: starter + single combining mark.
        // This is the most common case (precomposed Latin, Greek, Cyrillic, etc.)
        // and avoids one feed_entry_nfd call per character.
        if data.len() == 2 {
            let e0 = data[0];
            let ccc0 = (e0 >> tables::EXPANSION_CCC_SHIFT) as u8;
            if ccc0 == 0 {
                // First entry is a starter: flush previous state, set new starter.
                state.flush_nfd(out);
                let cp0 = e0 & tables::EXPANSION_CP_MASK;
                debug_assert!(cp0 <= 0x10FFFF && !(0xD800..=0xDFFF).contains(&cp0));
                state.current_starter = Some(unsafe { char::from_u32_unchecked(cp0) });
                // Second entry: combine directly without feed_entry_nfd overhead.
                let e1 = data[1];
                let cp1 = e1 & tables::EXPANSION_CP_MASK;
                let ccc1 = (e1 >> tables::EXPANSION_CCC_SHIFT) as u8;
                debug_assert!(cp1 <= 0x10FFFF && !(0xD800..=0xDFFF).contains(&cp1));
                let ch1 = unsafe { char::from_u32_unchecked(cp1) };
                if ccc1 != 0 {
                    state.ccc_buf.push(ch1, ccc1);
                } else {
                    // Both starters (rare): use general path for second entry.
                    state.feed_entry_nfd(ch1, 0, out);
                }
                return;
            }
        }
        // General expansion loop (3+ entries or first entry is non-starter).
        for &entry in data {
            let cp = entry & tables::EXPANSION_CP_MASK;
            let ccc = (entry >> tables::EXPANSION_CCC_SHIFT) as u8;
            debug_assert!(cp <= 0x10FFFF && !(0xD800..=0xDFFF).contains(&cp));
            let exp_ch = unsafe { char::from_u32_unchecked(cp) };
            state.feed_entry_nfd(exp_ch, ccc, out);
        }
        return;
    }

    // Singleton decomposition: the trie value's lower 16 bits are the BMP code point.
    let info = tv & 0xFFFF;
    debug_assert!(info <= 0xD7FF || (0xE000..=0xFFFF).contains(&info));
    let decomposed = unsafe { char::from_u32_unchecked(info) };
    let ccc = if info <= 0x7F {
        0
    } else {
        tables::lookup_ccc(decomposed)
    };
    state.feed_entry_nfd(decomposed, ccc, out);
}

// ---------------------------------------------------------------------------
// normalize_scalar -- fallback for short inputs
// ---------------------------------------------------------------------------

/// Normalize a string using pure scalar processing (no SIMD).
fn normalize_scalar<'a>(input: &'a str, form: Form) -> Cow<'a, str> {
    if input.is_empty() {
        return Cow::Borrowed(input);
    }

    // Quick-check: if the string is definitely already normalized, return early.
    if form.quick_check(input) == quick_check::IsNormalized::Yes {
        return Cow::Borrowed(input);
    }

    let mut out = String::with_capacity(input.len());
    let mut state = NormState::new();
    let mut decomp_buf = CccBuffer::new();

    for ch in input.chars() {
        process_char(ch, &mut state, &mut out, form, &mut decomp_buf);
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

    // Short inputs: use scalar path directly (includes quick_check).
    if len < 64 {
        return normalize_scalar(input, form);
    }

    // Single upfront quick-check. If definitely normalized, return early.
    let qc = form.quick_check(input);
    if qc == quick_check::IsNormalized::Yes {
        return Cow::Borrowed(input);
    }

    // QC = No or Maybe: allocate and normalize.
    let bound = form.passthrough_bound();
    let composes = form.composes();
    let mut out = String::with_capacity(form.estimated_capacity(len));
    let mut last_written: usize = 0;
    let mut state = NormState::new();
    let mut decomp_buf = CccBuffer::new();

    let mut pos: usize = 0;
    let ptr = bytes.as_ptr();

    // SIMD chunk loop.
    while pos + 64 <= len {
        let chunk_start = pos;

        // SAFETY: pos + 64 <= len, so ptr.add(pos) is valid for 64 bytes.
        // Prefetch pointers use wrapping_add because they may exceed the
        // allocation; prefetch is a non-faulting hint on all architectures.
        let mask = unsafe {
            let prefetch_l1 = ptr.wrapping_add(pos + prefetch::PREFETCH_L1_DISTANCE * prefetch::CHUNK_SIZE);
            let prefetch_l2 = ptr.wrapping_add(pos + prefetch::PREFETCH_L2_DISTANCE * prefetch::CHUNK_SIZE);
            simd::scan_and_prefetch(ptr.add(pos), prefetch_l1, prefetch_l2, bound)
        };

        if mask == 0 {
            // All passthrough: no bytes >= bound in this chunk.
            pos += 64;
            continue;
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

            // Decode the character at this position.
            let (ch, width) = utf8::decode_char_at(bytes, byte_pos);

            // Extended passthrough for decomposition-only forms (NFD/NFKD):
            // Non-decomposing starters (CCC=0) produce identical output, so
            // they can be bulk-copied with surrounding passthrough bytes,
            // avoiding per-character NormState flush + push overhead.
            if !composes {
                let cp = ch as u32;
                // CJK ideographs: guaranteed non-decomposing starters, no trie needed.
                if (cp >= 0x3400 && is_cjk_unified(cp))
                    || (cp >= 0x10000 && is_supp_safe(cp))
                {
                    continue;
                }
                // Hangul syllables: algorithmic decomposition, write jamo directly
                // to output bypassing per-entry NormState overhead.
                if hangul::is_hangul_syllable(ch) {
                    if byte_pos > last_written {
                        state.flush_nfd(&mut out);
                        out.push_str(&input[last_written..byte_pos]);
                    }
                    last_written = byte_pos + width;
                    state.flush_nfd(&mut out);
                    let (l, v, t) = hangul::decompose_hangul(ch);
                    out.push(l);
                    out.push(v);
                    if let Some(t_char) = t {
                        out.push(t_char);
                    }
                    continue;
                }
                // Non-CJK, non-Hangul: single trie lookup for both the
                // passthrough check and (if needed) decomposition processing.
                // Use unchecked supplementary path for cp >= 0x10000 (emoji etc).
                let tv = if cp >= 0x10000 {
                    // SAFETY: cp is a valid supplementary code point from a valid char.
                    unsafe { tables::raw_decomp_trie_value_supplementary(cp, form.decomp_form()) }
                } else {
                    tables::raw_decomp_trie_value(ch, form.decomp_form())
                };
                if !tables::has_decomposition(tv)
                    && tables::ccc_from_trie_value(tv) == 0
                {
                    continue; // non-decomposing starter → passthrough
                }
                // Needs work: copy passthrough, then process with inline NFD path.
                if byte_pos > last_written {
                    state.flush_nfd(&mut out);
                    out.push_str(&input[last_written..byte_pos]);
                }
                last_written = byte_pos + width;
                process_from_trie_nfd(
                    ch,
                    tv,
                    &mut state,
                    &mut out,
                    form.decomp_form(),
                );
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
            if byte_pos > last_written {
                state.flush(&mut out, composes);
                let pass = &input[last_written..byte_pos];
                let n = pass.len();
                if composes {
                    if n > 1 {
                        out.push_str(&pass[..n - 1]);
                    }
                    let last_ch = pass.as_bytes()[n - 1] as char;
                    state.feed_entry(last_ch, 0, &mut out, true);
                } else {
                    out.push_str(pass);
                }
            }

            last_written = byte_pos + width;

            // Process through decomposition + accumulation.
            process_char(ch, &mut state, &mut out, form, &mut decomp_buf);
        }

        pos += 64;
    }

    // Scalar tail: remaining bytes after the last full chunk.
    if pos < len {
        // Check if the tail has any non-passthrough bytes.
        let tail_has_work = bytes[pos..].iter().any(|&b| b >= bound);

        if tail_has_work {
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

                let (ch, width) = utf8::decode_char_at(bytes, tail_pos);

                // Extended passthrough (NFD/NFKD): skip non-decomposing starters.
                if !composes {
                    let cp = ch as u32;
                    if (cp >= 0x3400 && is_cjk_unified(cp))
                        || (cp >= 0x10000 && is_supp_safe(cp))
                    {
                        tail_pos += width;
                        continue;
                    }
                    // Hangul syllables: algorithmic decomposition, write directly.
                    if hangul::is_hangul_syllable(ch) {
                        if tail_pos > last_written {
                            state.flush_nfd(&mut out);
                            out.push_str(&input[last_written..tail_pos]);
                        }
                        last_written = tail_pos + width;
                        state.flush_nfd(&mut out);
                        let (l, v, t) = hangul::decompose_hangul(ch);
                        out.push(l);
                        out.push(v);
                        if let Some(t_char) = t {
                            out.push(t_char);
                        }
                        tail_pos += width;
                        continue;
                    }
                    let tv = if cp >= 0x10000 {
                        // SAFETY: cp is a valid supplementary code point from a valid char.
                        unsafe {
                            tables::raw_decomp_trie_value_supplementary(cp, form.decomp_form())
                        }
                    } else {
                        tables::raw_decomp_trie_value(ch, form.decomp_form())
                    };
                    if !tables::has_decomposition(tv)
                        && tables::ccc_from_trie_value(tv) == 0
                    {
                        tail_pos += width;
                        continue;
                    }
                    // Needs work: copy passthrough, process with inline NFD path.
                    if tail_pos > last_written {
                        state.flush_nfd(&mut out);
                        out.push_str(&input[last_written..tail_pos]);
                    }
                    last_written = tail_pos + width;
                    process_from_trie_nfd(
                        ch,
                        tv,
                        &mut state,
                        &mut out,
                        form.decomp_form(),
                    );
                    tail_pos += width;
                    continue;
                }

                // Copy passthrough bytes before this char.
                if tail_pos > last_written {
                    state.flush(&mut out, composes);
                    let pass = &input[last_written..tail_pos];
                    let n = pass.len();
                    if composes {
                        if n > 1 {
                            out.push_str(&pass[..n - 1]);
                        }
                        let last_ch = pass.as_bytes()[n - 1] as char;
                        state.feed_entry(last_ch, 0, &mut out, true);
                    } else {
                        out.push_str(pass);
                    }
                }

                last_written = tail_pos + width;

                process_char(ch, &mut state, &mut out, form, &mut decomp_buf);

                tail_pos += width;
            }
        }
    }

    // Flush any remaining state.
    if composes {
        state.flush(&mut out, true);
    } else {
        state.flush_nfd(&mut out);
    }

    // Copy any trailing passthrough bytes.
    if last_written < len {
        out.push_str(&input[last_written..len]);
    }

    // For the Maybe case (NFC/NFKC only), normalization might not have changed
    // anything. Check and return Borrowed if so.
    if qc == quick_check::IsNormalized::Maybe && out == input {
        Cow::Borrowed(input)
    } else {
        Cow::Owned(out)
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
