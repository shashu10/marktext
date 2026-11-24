---
name: migrate-slice
description: Execute an end-to-end Strangler-Fig migration slice from Electron/Vue to Rust/GPUI in MarkText (prerequisites, Before baseline, Draft PR, feature-flagged implementation, After scorecard, rollback check, and PR handoff). Use when asked to migrate a slice ID (e.g. A1-A8, W2.A, B1+) or work on a MarkText GPUI migration slice.
---

# Migrate Slice Workflow

Follow `docs/migration/RUNBOOK.md` and `docs/migration/MEMORY.md` to migrate one slice cleanly behind a feature flag.

## Checklist

```
Task Progress:
- [ ] 1. Verify prerequisites & claim slice on a branch off `develop`
- [ ] 2. Capture untouched "Before" baseline metrics & screenshot
- [ ] 3. Open Draft PR early with Before table + tentative plan
- [ ] 4. Implement Rust/GPUI crate + host bridge wiring behind feature flag
- [ ] 5. Verify build/tests/After metrics/rollback & inspect screenshots
- [ ] 6. Update STATUS.md, MEMORY.md (only if new trap discovered), mark PR ready, tag @shashu10
```

## Step 1: Check Prerequisites & Claim
1. Read `docs/migration/STATUS.md` and `docs/migration/MEMORY.md`.
2. Verify all slice prerequisites are merged and check `gh pr list --label migration` for parallel collisions.
3. Branch off latest `develop` and update the slice row in `docs/migration/STATUS.md` to `in-progress`.

## Step 2: Capture "Before" Baseline FIRST
Before writing migration code, build the untouched app and capture the legacy baseline:
```bash
pnpm build:unpack
xvfb-run --auto-servernum pnpm migration:measure --slice <SLICE_ID> --flag legacy \
  --out /tmp/<slice>-before.json --screenshot /tmp/<slice>-before.png
```
Never commit screenshots or JSON reports into git.

## Step 3: Open Early Draft PR
Push the `STATUS.md` claim and open a Draft PR targeting `develop` with label `migration`:
```bash
gh pr create --draft --base develop --label migration --title "feat(migration): migrate <SLICE_ID> (<Name>) to Rust/GPUI" --body "..."
```
Include the **Before baseline table** and **Tentative Implementation Plan** in the Draft PR body so they survive context compaction and lock the slice against parallel agents.

## Step 4: Implement Behind Feature Flag
- Target `gpui-ce` (`default-features = false`) and `gpui_ce_platform` (`default-features = false`) with no platform `#[cfg]` inside UI view crates.
- Route all host I/O through `HostBridge`; keep `prod` on `legacy` in `packages/desktop/src/shared/flags/environments.json`.
- Translate legacy controls and behavior first — do not mix opportunistic UX redesigns into a slice PR.
- Add Rust unit tests and Electron/IPC parity tests.

## Step 5: Verify & Measure After + Rollback
Follow the `verify-migration-slice` skill (or run its build/test/screenshot/rollback commands) and compute the deterministic progress delta:
```bash
pnpm migration:progress --slice <SLICE_ID>
```

## Step 6: Finalize PR & Request Review
1. Update the PR body with:
   - **Planned vs. Actually Implemented**
   - **Before vs. After scorecard** (distinguishing Stage 1 WASM overhead guardrails vs. Stage 2 native ROI)
   - **`pnpm migration:progress` output**
   - **Before / After / Rollback screenshot links** + visual inspection notes confirming no black/blank canvas or clipped labels
2. Add an entry to `docs/migration/MEMORY.md` **only** if an investigation uncovered a non-obvious constraint not already documented.
3. Update `docs/migration/STATUS.md` to `in-review`, run `gh pr ready`, tag `@shashu10`, and **never self-merge**.
