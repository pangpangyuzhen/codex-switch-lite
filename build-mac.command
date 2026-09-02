#!/bin/bash
set -euo pipefail

cd "$(dirname "$0")"

echo "== Codex Switch Lite / macOS build =="

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "This build script is intended for macOS."
  exit 1
fi

if ! xcode-select -p >/dev/null 2>&1; then
  echo "Xcode Command Line Tools not found. Run: xcode-select --install"
  exit 1
fi

if ! command -v node >/dev/null 2>&1; then
  echo "Node.js 20+ not found. Install Node.js, then rerun this script."
  exit 1
fi

NODE_MAJOR="$(node -p 'process.versions.node.split(`.`)[0]')"
if [[ "$NODE_MAJOR" -lt 20 ]]; then
  echo "Node.js 20+ is required. Current: $(node -v)"
  exit 1
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "Rust not found. Install with: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
  exit 1
fi

if ! command -v pnpm >/dev/null 2>&1; then
  if command -v corepack >/dev/null 2>&1; then
    corepack enable
    corepack prepare pnpm@10.12.3 --activate
  else
    echo "pnpm not found. Install it with: npm install -g pnpm@10.12.3"
    exit 1
  fi
fi

echo "Node: $(node -v)"
echo "pnpm: $(pnpm -v)"
echo "Rust: $(rustc --version)"

echo "Installing dependencies..."
pnpm install

echo "Type-checking frontend..."
pnpm typecheck

echo "Building macOS app..."
pnpm build

APP_PATH="src-tauri/target/release/bundle/macos/Codex Switch Lite.app"
if [[ -d "$APP_PATH" ]]; then
  echo
  echo "Build complete:"
  echo "$PWD/$APP_PATH"
  echo
  echo "To install:"
  echo "  cp -R \"$PWD/$APP_PATH\" /Applications/"
else
  echo "Build finished but app bundle was not found at the expected path."
  exit 1
fi
