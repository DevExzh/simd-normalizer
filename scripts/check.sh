#!/usr/bin/env bash
# scripts/check.sh -- Run the full CI check suite locally.

set -euo pipefail

echo "=== Format check ==="
cargo fmt --all -- --check

echo ""
echo "=== Clippy (default features) ==="
cargo clippy --all-targets -- -D warnings

echo ""
echo "=== Clippy (no_std + alloc) ==="
cargo clippy --no-default-features --features alloc --all-targets -- -D warnings

echo ""
echo "=== Tests (default features) ==="
cargo test --verbose

echo ""
echo "=== Tests (no_std + alloc) ==="
cargo test --no-default-features --features alloc --verbose

echo ""
echo "=== Documentation ==="
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features

echo ""
echo "=== Benchmark compilation ==="
cargo bench --no-run

echo ""
echo "=== All checks passed ==="
