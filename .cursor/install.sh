#!/usr/bin/env bash
set -euo pipefail

# Idempotent Cloud Agent Build script (runs from repo root during snapshot creation).
# Right-sized for measured container profile: 4-5 vCPU, 11-12 GiB RAM (<=8 GiB w/ lld + line-tables),
# 50 GiB disk, >=1 GiB /dev/shm, CARGO_BUILD_JOBS=3 (overridable via env).
export USE_ELECTRON_CLANG=0
export npm_config_user_agent=pnpm
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-3}"

echo "==> [1/6] Installing workspace dependencies (frozen lockfile)"
pnpm install --frozen-lockfile --ignore-scripts

echo "==> [2/6] Running MarkText postinstall (Electron binary, patches, native rebuild, locales, muya types)"
pnpm tsx scripts/postinstall.ts

echo "==> [3/6] Pre-building Electron desktop bundles"
pnpm build:unpack

# Note: Desktop E2E tests and scripts/migration-measure.ts launch Electron's bundled Chromium
# via playwright._electron.launch(), so standalone Playwright Chromium (~450 MB) is intentionally
# skipped to keep snapshot size and build time lean. Run `pnpm -C packages/muya/e2e exec playwright
# install chromium` only if working on packages/muya/e2e browser suites.

echo "==> [4/6] Warming Rust crates & Stage 1 GPUI WASM bundle"
if [ -f "rust/Cargo.toml" ]; then
  cargo fetch --manifest-path rust/Cargo.toml
  pnpm migration:build-wasm

  echo "==> [5/6] Warming Stage 2 native GPUI binary (mt-pref-app) & Rust unit test cache"
  pnpm migration:build-native-pref
  cargo test --manifest-path rust/Cargo.toml --no-run
else
  echo "No rust/Cargo.toml present; skipping Rust warm-up."
fi

echo "==> [6/6] Pruning redundant package caches"
pnpm store prune || true

echo "==> Cloud Agent environment Build complete."
