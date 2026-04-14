// tests/api.rs
use std::borrow::Cow;

#[test]
fn test_nfc_constructor_exists() {
    let norm = simd_normalizer::nfc();
    let result = norm.normalize("hello");
    assert_eq!(result, "hello");
}

#[test]
fn test_nfd_constructor_exists() {
    let norm = simd_normalizer::nfd();
    let result = norm.normalize("hello");
    assert_eq!(result, "hello");
}

#[test]
fn test_nfkc_constructor_exists() {
    let norm = simd_normalizer::nfkc();
    let result = norm.normalize("hello");
    assert_eq!(result, "hello");
}

#[test]
fn test_nfkd_constructor_exists() {
    let norm = simd_normalizer::nfkd();
    let result = norm.normalize("hello");
    assert_eq!(result, "hello");
}

#[test]
fn test_ascii_returns_borrowed() {
    let input = "The quick brown fox jumps over the lazy dog.";
    let result = simd_normalizer::nfc().normalize(input);
    match &result {
        Cow::Borrowed(s) => assert!(core::ptr::eq(*s, input)),
        Cow::Owned(_) => panic!("expected Cow::Borrowed for pure ASCII"),
    }
}

#[test]
fn test_trait_on_str() {
    use simd_normalizer::UnicodeNormalization;

    let input = "\u{00C5}\u{03A9}";
    let nfc_result = input.nfc();
    let nfd_result = input.nfd();
    let _nfkc_result = input.nfkc();
    let _nfkd_result = input.nfkd();

    assert_eq!(&*nfc_result, input);
    assert_ne!(&*nfd_result, input);
    assert!(input.is_nfc());
    assert!(!nfd_result.is_nfc());

    assert_eq!(&*input.nfc(), &*simd_normalizer::nfc().normalize(input));
    assert_eq!(&*input.nfd(), &*simd_normalizer::nfd().normalize(input));
    assert_eq!(&*input.nfkc(), &*simd_normalizer::nfkc().normalize(input));
    assert_eq!(&*input.nfkd(), &*simd_normalizer::nfkd().normalize(input));
}

#[test]
fn test_is_normalized_enum_exposed() {
    use simd_normalizer::IsNormalized;
    let _yes = IsNormalized::Yes;
    let _no = IsNormalized::No;
    let _maybe = IsNormalized::Maybe;
}

#[test]
fn test_normalize_to_buffer() {
    let norm = simd_normalizer::nfc();
    let mut buf = String::new();
    let was_normalized = norm.normalize_to("\u{0041}\u{030A}", &mut buf);
    assert!(!was_normalized);
    assert_eq!(buf, "\u{00C5}");

    buf.clear();
    let was_normalized = norm.normalize_to("hello", &mut buf);
    assert!(was_normalized);
    assert_eq!(buf, "hello");
}

#[test]
fn test_quick_check_method() {
    use simd_normalizer::IsNormalized;
    let norm = simd_normalizer::nfc();
    let result = norm.quick_check("hello world");
    assert_eq!(result, IsNormalized::Yes);
}
