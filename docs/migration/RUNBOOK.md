# Cloud Agent Migration Runbook (Slice Execution Protocol)

Every Cursor Cloud Agent (or SDK-dispatched agent) executing a migration slice from `docs/migration/STATUS.md` **must** follow this exact sequence. This guarantees comparable Before/After benchmarks, preserves baseline data across context compaction via an early Draft PR, prevents parallel-agent slice collisions, and enforces human approval before merge.

---

## Step 1 — Orient, Verify Prerequisites & Claim Slice

1. Read `AGENTS.md`, `docs/migration/ARCHITECTURE.md`, `docs/migration/MIGRATION_PLAN.md`, and `docs/migration/STATUS.md`.
2. Check prerequisites for your target slice in `STATUS.md`:
   - If any prerequisite is not `verified`, **stop and report the blocker** rather than inventing ad-hoc scaffolding.
   - If the prompt asked for _"the next unmigrated slice"_ without naming an ID, pick the first `todo` row whose prerequisites are all `verified` **and** that has no open PR (`gh pr list --repo shashu10/marktext --label migration --state open`).
3. Create a clean branch from `develop`:
   ```bash
   git checkout develop && git pull origin develop
   git checkout -b migration/<slice-id>-<short-name>
   ```
   Example: `migration/a1-pref-general`.
4. Update the slice's row in `docs/migration/STATUS.md` from `todo` → `in-progress`.

---

## Step 2 — Establish the "Before" Baseline (Untouched Legacy App)

**Measure BEFORE writing any migration code** so the baseline reflects the unmodified legacy behavior on the exact same machine.

1. Build the unmodified app (`pnpm build:unpack`).
2. Run the deterministic measurement harness (when `P0.4` is merged):
   ```bash
   xvfb-run --auto-servernum pnpm migration:measure \
     --slice <slice-id> \
     --phase before \
     --out /tmp/<slice-id>-before.json \
     --screenshot /tmp/<slice-id>-before.png
   ```
   _(If running before `P0.4` exists, capture the same metrics using Playwright under `xvfb-run --auto-servernum` and Electron `app.getAppMetrics()`, saving structured JSON to `/tmp/<slice-id>-before.json` and PNG to `/tmp/<slice-id>-before.png`.)_
3. Required baseline metrics to record:
   - **Startup / window-ready latency (ms):** Cold launch to `ready-to-show` + target pane interactive.
   - **Pane render / switch latency (ms):** Time from opening or switching to the target pane until first painted frame.
   - **Memory footprint (MB RSS):** Electron Main process + Renderer process(es) + GPU process (`app.getAppMetrics()`).
   - **Idle CPU usage (%):** Average CPU % across a 5-second idle window after the target pane stabilizes.
   - **Artifact / bundle size (KB/MB):** Renderer JS/CSS bundle size (and WASM/native binary size when present).
   - **Before screenshot:** `/tmp/<slice-id>-before.png` (keep outside tracked git tree!).

---

## Step 3 — Open a Draft PR Immediately (Context-Preservation Anchor)

Do **not** wait until implementation finishes to record baseline data. Commit the `STATUS.md` claim and open a **Draft PR** right away so your baseline metrics and tentative plan survive context summarization and visibly lock the slice against parallel agents:

```bash
git add docs/migration/STATUS.md
git commit -m "chore(migration): claim slice <slice-id> and record baseline"
git push -u origin migration/<slice-id>-<short-name>
gh pr create --draft \
  --repo shashu10/marktext \
  --base develop \
  --label migration \
  --title "feat(migration): migrate <slice-id> (<component-name>) to Rust/GPUI" \
  --body-file /tmp/<slice-id>-pr-draft.md
```

The Draft PR body must include:

- **Slice ID, Feature Flag Key, and Stage** (`Stage 1: gpui_web WASM` vs. `Stage 2: native GPUI`).
- **Tentative Implementation Plan:** exact preference keys / UI controls / `HostBridge` calls / tests planned.
- **Before Baseline Table:** the raw measurements from `/tmp/<slice-id>-before.json`.
- **Placeholder sections** for _Planned vs. Implemented_, _After Measurements_, _Screenshots_, and _Verification Checklist_.

---

## Step 4 — Implement the Migrated Slice

1. Implement the Rust GPUI view in the target crate listed in `docs/migration/STATUS.md` using `gpui-ce` (`default-features = false`) + `gpui_ce_platform` (`default-features = false`).
2. Route all preference reads/writes, file metadata, and editor commands through `HostBridge` — never disconnected local state.
3. Gate the migrated slice behind its feature flag in `flags/environments.json` (`legacy` | `rust-wasm` | `gpui-native` | `shadow`), keeping `prod` on `legacy` until explicitly promoted.
4. Preserve full functional parity:
   - Preference changes persist to `preferences.json` and broadcast `mt::user-preference` to open Editor windows.
   - Preferences global search (`registry.ts` / `schema.json`) still resolves controls in the migrated pane.
   - Editor panels (TOC, Document Stats, Find/Replace) work in **both** WYSIWYG and Source Code modes.
   - Toggling the feature flag back to `legacy` cleanly restores the Vue component with zero state corruption.
5. Add automated tests:
   - Rust unit tests (`cargo test`) covering state logic & schema round-trip.
   - Bridge / parity tests covering Electron Main IPC persistence & rollback.

---

## Step 5 — Validate & Collect "After" Measurements

Run **all** deterministic checks first; do not publish a ready PR if any check fails:

```bash
pnpm run lint
pnpm run typecheck
pnpm test:unit
cargo test --manifest-path rust/Cargo.toml
cargo clippy --manifest-path rust/Cargo.toml -- -D warnings
cargo build --manifest-path rust/Cargo.toml --target wasm32-unknown-unknown --release
```

Then run the **exact same measurement harness** with the slice's feature flag enabled:

```bash
xvfb-run --auto-servernum pnpm migration:measure \
  --slice <slice-id> \
  --phase after \
  --flag rust-wasm \
  --out /tmp/<slice-id>-after.json \
  --screenshot /tmp/<slice-id>-after.png \
  --compare /tmp/<slice-id>-before.json
```

Also run a **rollback sanity check** with `--flag legacy` to confirm instant safe rollback.

### Honest Stage 1 vs. Stage 2 Performance Interpretation

- **Stage 1 (`gpui_web` WASM inside Electron):** Chromium is still running alongside the WASM module, so RSS memory and bundle size may be temporarily flat or higher. Treat Stage 1 numbers as **overhead guardrails** (acceptable transient cost while proving 100% functional + visual parity).
- **Stage 2 (`gpui-native` full window flip):** Chromium is retired for that window; report the true enterprise ROI delta (startup latency, RSS memory drop, native binary vs. Electron window footprint).

---

## Step 6 — Finalize PR Description, Attach Screenshots & Request Human Review

1. Update `docs/migration/STATUS.md` row from `in-progress` → `in-review` and push all commits.
2. **Screenshots:** Never commit PNGs into the repository tree. Attach Before & After screenshots via:
   - Cursor Cloud Agent **Artifacts** (include the Cloud Agent run URL in the PR), and/or
   - The CI/artifact upload URL configured by the QA workflow so images render inline in the PR.
3. Update the PR description (`gh pr edit --body-file /tmp/<slice-id>-pr-final.md`) using the template below, explicitly comparing **Tentative Plan vs. Actually Implemented** and **Before vs. After**.
4. Mark the PR ready and tag `@shashu10` for human approval:
   ```bash
   gh pr ready --repo shashu10/marktext
   gh pr comment --repo shashu10/marktext --body "@shashu10 Slice \`<slice-id>\` is implemented, verified, and ready for human review. All automated checks and Before/After measurements are in the PR description."
   ```
   _(Note: When the Cloud Agent authenticates under `shashu10`, GitHub disallows adding `shashu10` via `--add-reviewer` on their own PR; tagging `@shashu10` in the PR body/comment + enforcing required CI status checks gates the merge. In enterprise production rollouts, Cloud Agents authenticate as a dedicated GitHub App bot so GitHub enforces `CODEOWNERS` human approval natively.)_
5. **Never merge the PR.** Stop and hand off to the human reviewer + automated QA Verifier CI check.

---

## Standard PR Description Template

```markdown
## Migration Slice

- **Slice ID:** `<slice-id>`
- **Component:** `<name>`
- **Feature Flag:** `<flag-key>` (`dev`: `rust-wasm`, `staging`: `shadow`, `prod`: `legacy`)
- **Migration Stage:** Stage 1 (`gpui_web` WASM) | Stage 2 (`gpui-native`)
- **Cloud Agent Run / Artifacts:** `<url-if-available>`

## Tentative Plan (Recorded at Baseline)

- Controls / behaviors to port:
- `HostBridge` calls required:
- Acceptance & parity criteria:

## Planned vs. Actually Implemented

- **Delivered as planned:** ...
- **Deviations / surprises encountered:** ...
- **Follow-ups / known limitations:** ...

## Before vs. After Measurements

| Metric                             | Before (Legacy Electron/Vue) | After (Migrated) | Delta | Notes                  |
| ---------------------------------- | ---------------------------: | ---------------: | ----: | ---------------------- |
| Window / Startup Latency (ms)      |                              |                  |       |                        |
| Pane Open / Switch Latency (ms)    |                              |                  |       |                        |
| Memory — Main + Renderer RSS (MB)  |                              |                  |       |                        |
| Idle CPU (5s avg, %)               |                              |                  |       |                        |
| Renderer / WASM / Native Size (MB) |                              |                  |       |                        |
| Visual Diff vs. Legacy (%)         |                           0% |                  |       |                        |
| Functional Parity & Rollback Check |                         pass |                  |       | `flag=legacy` verified |

## AI SDLC Run Telemetry

- **Wall-clock duration:** `<Xm Ys>`
- **Test/build retry count:** `<N>`
- **Checks run:** `pnpm lint`, `pnpm typecheck`, `pnpm test:unit`, `cargo test`, `cargo clippy`, `wasm32` build, `xvfb-run` Playwright parity

## Screenshots (Before vs. After)

| Before (Legacy)             | After (Rust/GPUI)          |
| --------------------------- | -------------------------- |
| `![before](<artifact-url>)` | `![after](<artifact-url>)` |

## Review & Approval

- Tagging **@shashu10** for human review and LGTM. Do not auto-merge.
```
