#!/usr/bin/env bash
set -euo pipefail

# Idempotent Cloud Agent Build script (runs from repo root during Build creation).
export USE_ELECTRON_CLANG=0
export npm_config_user_agent=pnpm

echo "==> [1/5] Installing workspace dependencies (frozen lockfile)"
pnpm install --frozen-lockfile --ignore-scripts

echo "==> [2/5] Running MarkText postinstall (Electron binary, patches, native rebuild, locales, muya types)"
pnpm tsx scripts/postinstall.ts

echo "==> [3/5] Pre-building Electron desktop bundles"
pnpm build:unpack

echo "==> [4/5] Ensuring Playwright Chromium is installed for visual QA"
pnpm -C packages/desktop exec playwright install chromium

echo "==> [5/5] Warming Rust/GPUI WASM cache (if Rust workspace is present)"
if [ -f "rust/Cargo.toml" ]; then
  cargo fetch --manifest-path rust/Cargo.toml
  cargo build --manifest-path rust/Cargo.toml --target wasm32-unknown-unknown --release
elif [ -f "Cargo.toml" ]; then
  cargo fetch
  cargo build --target wasm32-unknown-unknown --release
else
  echo "No Rust workspace present yet; skipping Cargo warm-up."
fi

echo "==> Cloud Agent environment Build complete."
