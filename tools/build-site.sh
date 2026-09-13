#!/usr/bin/env bash
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
web="$repo/playground/web"
dist="$repo/playground/dist"

cargo run --quiet --manifest-path "$repo/tools/extract-docs/Cargo.toml" -- --out "$web/data"
(
  cd "$repo/playground"
  wasm-pack build --target web --release --out-dir web/pkg
)
mkdir -p "$dist/pkg" "$dist/data" "$dist/assets"
cp "$web/index.html" "$web/styles.css" "$web/terminal.css" "$web/app.js" "$web/features.js" "$web/stage.js" "$web/docs.js" "$dist/"
cp "$web/data/features.json" "$web/data/docs.json" "$dist/data/"
cp "$web/pkg/textflow_playground.js" "$web/pkg/textflow_playground_bg.wasm" "$dist/pkg/"
cp "$repo/assets/textflow-logo.svg" "$repo/assets/textflow-icon.svg" "$dist/assets/"
printf 'Built %s\n' "$dist"
