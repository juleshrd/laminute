#!/usr/bin/env bash
# Smoke E2E natif Linux (JUL-204) — sans clé API ni micro.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "==> e2e_smoke (bibliothèque + fixture MP3)"
cargo run --manifest-path src-tauri/Cargo.toml --example e2e_smoke

if [[ "${1:-}" == "--release-bundle" ]]; then
  echo "==> build bundle (peut être long)"
  npm run build:web
  # Smoke post-build : vérifier que le binaire release existe après build:rust
  cargo build --manifest-path src-tauri/Cargo.toml --release
  BIN=src-tauri/target/release/laminute
  test -x "$BIN"
  echo "binaire release présent : $BIN ($(stat -c '%s' "$BIN") octets)"
fi

echo "OK e2e"
