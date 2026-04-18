# simd-normalizer fuzz

Coverage-guided fuzz tests for `simd-normalizer` public APIs.

The fuzz crate is an opt-in standalone cargo package — it does not affect
parent builds. It is kept out of the parent workspace (see `[workspace]` at
the bottom of `Cargo.toml`) so its `std`-using dependencies do not leak into
the core `no_std + alloc` library.

## Prerequisites

- `cargo install cargo-fuzz`
- Rust nightly toolchain: `rustup install nightly`

## Running

From the `fuzz/` directory:

```
cargo +nightly fuzz run <target>
```

For example:

```
cargo +nightly fuzz run fuzz_nfc
```

## Targets

- `fuzz_nfc`, `fuzz_nfd`, `fuzz_nfkc`, `fuzz_nfkd` — per-form
  normalize / is_normalized / quick_check plus idempotence.
- `fuzz_matching` — matching pipeline (both case fold modes),
  reflexivity, UTF-16 round-trip.
- `fuzz_casefold` — casefold (string and char, both modes), idempotence.
- `fuzz_confusable` — skeleton idempotence, `are_confusable` reflexivity
  and symmetry.
- `fuzz_cross_form` — NFC / NFD / NFKC / NFKD cross-form compositional
  invariants.
- `fuzz_differential` — byte-for-byte comparison against the
  `unicode-normalization` reference crate.
