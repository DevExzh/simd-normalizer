// tests/thread_safety.rs
//
// Thread safety and no_std verification for simd-normalizer.
//
// Tests concurrent normalization from multiple threads, verifies Send + Sync
// trait bounds at compile time, and documents no_std + alloc support.

use std::sync::Arc;
use std::thread;

use simd_normalizer::{
    nfc, nfd, nfkc, nfkd, CaseFoldMode, IsNormalized, MatchingOptions, NfcNormalizer,
    NfdNormalizer, NfkcNormalizer, NfkdNormalizer, UnicodeNormalization,
};

// =========================================================================
// Test 1: Concurrent normalization from 8 threads
// =========================================================================

/// Each thread normalizes its assigned input through NFC, NFD, NFKC, NFKD
/// for 1000 iterations. All threads must produce correct results with no panics.
#[test]
fn concurrent_normalization_8_threads() {
    // Thread inputs -- each exercises a different script / text category.
    let inputs: &[(&str, &str)] = &[
        ("ascii", "The quick brown fox jumps over the lazy dog. 0123456789!"),
        ("cjk", "\u{4E16}\u{754C}\u{4F60}\u{597D}\u{6D4B}\u{8BD5}\u{6587}\u{672C}"),
        ("emoji", "\u{1F600}\u{1F60D}\u{1F389}\u{1F680}\u{2764}\u{FE0F}\u{1F308}\u{1F3B6}"),
        (
            "combining",
            "A\u{0300}\u{0301}\u{0302}e\u{0308}\u{0304}o\u{0327}\u{0328}\u{030A}",
        ),
        (
            "hangul",
            "\u{AC00}\u{B098}\u{B2E4}\u{B77C}\u{B9C8}\u{BC14}\u{C0AC}\u{C544}\u{C790}\u{CC28}",
        ),
        (
            "arabic",
            "\u{0627}\u{0644}\u{0639}\u{0631}\u{0628}\u{064A}\u{0629}\u{0020}\u{0641}\u{062D}\u{0635}",
        ),
        (
            "mixed",
            "Hello\u{4E16}\u{754C}\u{0410}\u{043B}\u{043B}\u{043E}\u{3053}\u{3093}\u{306B}\u{3061}\u{306F}",
        ),
        (
            "long",
            &"A\u{0300}\u{0301}B\u{0327}C\u{030A}D\u{0308}E\u{0303}F\u{0304}G\u{0306}H\u{030C}"
                .repeat(50),
        ),
    ];

    // Pre-compute expected results for verification.
    let expected: Vec<(String, String, String, String)> = inputs
        .iter()
        .map(|(_, input)| {
            (
                nfc().normalize(input).into_owned(),
                nfd().normalize(input).into_owned(),
                nfkc().normalize(input).into_owned(),
                nfkd().normalize(input).into_owned(),
            )
        })
        .collect();

    let inputs_arc: Arc<Vec<(String, String, String, String)>> = Arc::new(expected);
    let raw_inputs: Arc<Vec<String>> = Arc::new(
        inputs.iter().map(|(_, s)| s.to_string()).collect(),
    );

    let handles: Vec<_> = (0..8)
        .map(|i| {
            let expected = Arc::clone(&inputs_arc);
            let raw = Arc::clone(&raw_inputs);
            thread::spawn(move || {
                for _ in 0..1000 {
                    let input = &raw[i];
                    let (exp_nfc, exp_nfd, exp_nfkc, exp_nfkd) = &expected[i];

                    let got_nfc = nfc().normalize(input);
                    let got_nfd = nfd().normalize(input);
                    let got_nfkc = nfkc().normalize(input);
                    let got_nfkd = nfkd().normalize(input);

                    assert_eq!(&*got_nfc, exp_nfc.as_str());
                    assert_eq!(&*got_nfd, exp_nfd.as_str());
                    assert_eq!(&*got_nfkc, exp_nfkc.as_str());
                    assert_eq!(&*got_nfkd, exp_nfkd.as_str());
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("thread panicked during concurrent normalization");
    }
}

// =========================================================================
// Test 2: Concurrent is_normalized + normalize
// =========================================================================

/// Spawn threads that call `is_normalized()` and `normalize()` on the same
/// shared input simultaneously. Verifies no data races or panics.
#[test]
fn concurrent_is_normalized_and_normalize() {
    let inputs: &[&str] = &[
        "hello world",
        "\u{00C5}\u{03A9}",
        "A\u{0300}\u{0301}B\u{0327}",
        "\u{AC00}\u{B098}\u{B2E4}",
        "\u{FB01}\u{FB02}\u{2126}",
        "\u{1F600}\u{200D}\u{1F525}",
    ];

    let shared: Arc<Vec<String>> = Arc::new(inputs.iter().map(|s| s.to_string()).collect());

    let mut handles = Vec::new();

    // Spawn normalizer threads.
    for _ in 0..4 {
        let data = Arc::clone(&shared);
        handles.push(thread::spawn(move || {
            for _ in 0..500 {
                for input in data.iter() {
                    let _ = nfc().normalize(input.as_str());
                    let _ = nfd().normalize(input.as_str());
                    let _ = nfkc().normalize(input.as_str());
                    let _ = nfkd().normalize(input.as_str());
                }
            }
        }));
    }

    // Spawn is_normalized threads.
    for _ in 0..4 {
        let data = Arc::clone(&shared);
        handles.push(thread::spawn(move || {
            for _ in 0..500 {
                for input in data.iter() {
                    let _ = nfc().is_normalized(input.as_str());
                    let _ = nfd().is_normalized(input.as_str());
                    let _ = nfkc().is_normalized(input.as_str());
                    let _ = nfkd().is_normalized(input.as_str());
                }
            }
        }));
    }

    for h in handles {
        h.join().expect("thread panicked during concurrent is_normalized + normalize");
    }
}

// =========================================================================
// Test 3: Send + Sync static assertions
// =========================================================================

/// Compile-time verification that all public normalizer types implement Send.
#[test]
fn send_impls() {
    fn assert_send<T: Send>() {}

    assert_send::<NfcNormalizer>();
    assert_send::<NfdNormalizer>();
    assert_send::<NfkcNormalizer>();
    assert_send::<NfkdNormalizer>();
    assert_send::<IsNormalized>();
    assert_send::<CaseFoldMode>();
    assert_send::<MatchingOptions>();
}

/// Compile-time verification that all public normalizer types implement Sync.
#[test]
fn sync_impls() {
    fn assert_sync<T: Sync>() {}

    assert_sync::<NfcNormalizer>();
    assert_sync::<NfdNormalizer>();
    assert_sync::<NfkcNormalizer>();
    assert_sync::<NfkdNormalizer>();
    assert_sync::<IsNormalized>();
    assert_sync::<CaseFoldMode>();
    assert_sync::<MatchingOptions>();
}

/// Combined Send + Sync assertion using a single generic bound, ensuring
/// the types can be shared across threads and moved between them.
#[test]
fn send_and_sync_combined() {
    fn assert_send_sync<T: Send + Sync>() {}

    assert_send_sync::<NfcNormalizer>();
    assert_send_sync::<NfdNormalizer>();
    assert_send_sync::<NfkcNormalizer>();
    assert_send_sync::<NfkdNormalizer>();
    assert_send_sync::<IsNormalized>();
    assert_send_sync::<CaseFoldMode>();
    assert_send_sync::<MatchingOptions>();
}

// =========================================================================
// Test 4: no_std + alloc compilation check
// =========================================================================

/// Documents that the crate supports `no_std + alloc`.
///
/// Verified by running:
///     cargo check --no-default-features --features alloc
///
/// This test exists as documentation that no_std is supported.
/// The actual verification is done at build time.
#[test]
fn nostd_alloc_compiles() {
    // Verified by: cargo check --no-default-features --features alloc
    // This test exists as documentation that no_std is supported.
    // The actual verification is done at build time.
    //
    // See also the CI pipeline which runs this check.
}

// =========================================================================
// Additional thread safety: UnicodeNormalization trait from multiple threads
// =========================================================================

/// Verify the `UnicodeNormalization` convenience trait methods work correctly
/// when called from multiple threads simultaneously.
#[test]
fn concurrent_trait_methods() {
    let input = Arc::new(String::from(
        "\u{00C5}\u{03A9}\u{FB01}A\u{0300}\u{0301}",
    ));

    let expected_nfc = input.as_str().nfc().into_owned();
    let expected_nfd = input.as_str().nfd().into_owned();
    let expected_nfkc = input.as_str().nfkc().into_owned();
    let expected_nfkd = input.as_str().nfkd().into_owned();

    let handles: Vec<_> = (0..4)
        .map(|_| {
            let inp = Arc::clone(&input);
            let exp_nfc = expected_nfc.clone();
            let exp_nfd = expected_nfd.clone();
            let exp_nfkc = expected_nfkc.clone();
            let exp_nfkd = expected_nfkd.clone();
            thread::spawn(move || {
                for _ in 0..500 {
                    assert_eq!(&*inp.as_str().nfc(), exp_nfc.as_str());
                    assert_eq!(&*inp.as_str().nfd(), exp_nfd.as_str());
                    assert_eq!(&*inp.as_str().nfkc(), exp_nfkc.as_str());
                    assert_eq!(&*inp.as_str().nfkd(), exp_nfkd.as_str());
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("thread panicked during concurrent trait method calls");
    }
}
