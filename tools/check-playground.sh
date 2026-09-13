#!/usr/bin/env bash
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo"

cargo fmt --all --check
cargo fmt --manifest-path playground/Cargo.toml --check
cargo fmt --manifest-path tools/extract-docs/Cargo.toml --check
cargo test
cargo test --all-features
cargo clippy --all-features --all-targets -- -D warnings
cargo test --manifest-path playground/Cargo.toml
cargo clippy --manifest-path playground/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path tools/extract-docs/Cargo.toml
cargo clippy --manifest-path tools/extract-docs/Cargo.toml --all-targets -- -D warnings
cargo run --manifest-path playground/Cargo.toml --bin scaffold_check
"$repo/tools/build-site.sh"
for script in "$repo/playground/web/"{app,features,stage,docs}.js; do node --experimental-default-type=module --check "$script"; done
node --experimental-default-type=module "$repo/playground/web/smoke.mjs"
