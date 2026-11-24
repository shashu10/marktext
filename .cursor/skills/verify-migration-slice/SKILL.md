---
name: verify-migration-slice
description: Build, test, benchmark, visually inspect screenshots, and verify legacy rollback for a MarkText Rust/GPUI migration slice (Stage 1 WASM and Stage 2 Native). Use after implementing or fixing a migration slice, before marking a migration PR ready for review, or when validating regressions.
---

# Verify Migration Slice

Run the full verification loop for a Stage 1 (`rust-wasm`) or Stage 2 (`gpui-native`) migration slice on Linux Cloud Agent VMs or macOS.

## 1. Build Artifacts
Build whichever targets the slice touches:
```bash
pnpm build:unpack
pnpm migration:build-wasm          # Stage 1 WASM host + wasm-bindgen glue
pnpm migration:build-native-pref   # Stage 2 native GPUI binary (mt-pref-app)
```

## 2. Static Checks & Unit Tests
```bash
pnpm lint
pnpm typecheck
pnpm test:unit
cargo test --manifest-path rust/Cargo.toml
cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings
```
All commands must exit `0` with zero warnings and zero skipped/weakened tests.

## 3. Headless Benchmark, Screenshots & Rollback Check
On Linux Cloud Agent VMs, always wrap Electron/GPUI runs in `xvfb-run --auto-servernum` (Lavapipe Vulkan ICD is preconfigured at `/usr/share/vulkan/icd.d/lvp_icd.json`):
```bash
# After (Stage 1 WASM or Stage 2 Native)
xvfb-run --auto-servernum pnpm migration:measure --slice <SLICE_ID> --flag <rust-wasm|gpui-native> \
  --out /tmp/<slice>-after.json --screenshot /tmp/<slice>-after.png

# Rollback verification (must match untouched legacy behavior)
xvfb-run --auto-servernum pnpm migration:measure --slice <SLICE_ID> --flag legacy \
  --out /tmp/<slice>-rollback.json --screenshot /tmp/<slice>-rollback.png
```
On macOS hardware GPU hosts, run `pnpm migration:measure ...` directly (without `xvfb-run`) to capture native Metal metrics.

## 4. Mandatory Visual Screenshot Inspection
Open `/tmp/<slice>-before.png`, `/tmp/<slice>-after.png`, and `/tmp/<slice>-rollback.png` and verify:
- [ ] Canvas is painted (not black, blank, or stuck on a placeholder)
- [ ] All controls, headings, and labels are legible with no clipping or overlap
- [ ] Active category / selected states and scrollbars behave identically to legacy
- [ ] Rollback screenshot matches the legacy Before screenshot

## 5. Progress Delta & Clean Tree Check
```bash
pnpm migration:progress --slice <SLICE_ID>
git status -sb
```
Confirm no PNGs, `/tmp` artifacts, or generated `gpui-host/*.wasm` / `*.js` files are staged or untracked inside the repository.
