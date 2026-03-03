#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

# Zed extensions directory
ZED_EXT_DIR="${HOME}/.local/share/zed/extensions/installed/zed-rest"

cd "$PROJECT_DIR"

# Build first
bash scripts/build.sh

echo ""
echo "==> Installing to Zed extensions directory..."
mkdir -p "$ZED_EXT_DIR/languages/http"

# Copy extension manifest
cp extension.toml "$ZED_EXT_DIR/"

# Copy WASM binary
cp target/wasm32-wasip1/release/zed_rest.wasm "$ZED_EXT_DIR/"

# Copy language config and queries
cp languages/http/config.toml "$ZED_EXT_DIR/languages/http/"
cp languages/http/highlights.scm "$ZED_EXT_DIR/languages/http/"
cp languages/http/injections.scm "$ZED_EXT_DIR/languages/http/"
cp languages/http/outline.scm "$ZED_EXT_DIR/languages/http/"

# Copy native LSP binary
cp target/release/rest-cli "$ZED_EXT_DIR/"

echo ""
echo "Installed to $ZED_EXT_DIR"
echo "Restart Zed to load the extension."
