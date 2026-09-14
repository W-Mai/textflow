#!/usr/bin/env bash
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo"

cargo fmt --all --check
cargo fmt --manifest-path playground/Cargo.toml --check
cargo fmt --manifest-path tools/extract-docs/Cargo.toml --check
cargo fmt --manifest-path tools/xtask/Cargo.toml --check
cargo fmt --manifest-path tools/budget-probe/Cargo.toml --check
cargo test
cargo test --all-features
cargo clippy --all-features --all-targets -- -D warnings
cargo test --manifest-path playground/Cargo.toml
cargo clippy --manifest-path playground/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path tools/extract-docs/Cargo.toml
cargo clippy --manifest-path tools/extract-docs/Cargo.toml --all-targets -- -D warnings
cargo clippy --manifest-path tools/xtask/Cargo.toml --all-targets -- -D warnings
cargo clippy --manifest-path tools/budget-probe/Cargo.toml --target wasm32-unknown-unknown --all-features -- -D warnings
cargo run --manifest-path playground/Cargo.toml --bin scaffold_check
"$repo/tools/build-site.sh"
node_command=(node)
if node --experimental-default-type=module --eval "" >/dev/null 2>&1; then
  node_command+=(--experimental-default-type=module)
fi
for script in "$repo/playground/web/"{app,features,stage,docs,code-highlight,budget,baseline-examples,baseline-code,atmosphere}.js; do "${node_command[@]}" --check "$script"; done
"${node_command[@]}" "$repo/playground/web/smoke.mjs"
"${node_command[@]}" --check "$repo/tools/budget.mjs"
"${node_command[@]}" "$repo/tools/budget.mjs" --check
