#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_DIR"

echo "==> Building WASM extension..."
cargo build --release --target wasm32-wasip1 -p zed-rest

echo "==> Building native LSP server..."
cargo build --release -p zed-rest-lsp

echo ""
echo "Build complete!"
echo "  WASM: target/wasm32-wasip1/release/zed_rest.wasm"
echo "  LSP:  target/release/zed-rest-lsp"
