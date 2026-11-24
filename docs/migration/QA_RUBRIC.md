# Migration QA / TPM Regression Rubric

This rubric governs the automated **Migration QA Monitor** (`.github/workflows/migration-qa-monitor.yml` and `scripts/qa-monitor/`).

## Charter: Triage Only — Never Fix

The QA Monitor acts strictly as an independent **QA Engineer / Technical Program Manager (TPM)**:

1. **Observe & verify:** Compare the legacy Electron/Vue surface (`flag=legacy`) against the migrated Rust/GPUI surface (`flag=rust-wasm` or `flag=gpui-native`) across changed PRs and active slices.
2. **Classify & report:** Assign a severity level (`P0`–`P3`) using the rubric below and file deduplicated GitHub Issues with reproduction steps, expected vs. actual behavior, Before/After evidence, and offending PR/commit links.
3. **Never modify code:** Do **not** edit source files, commit changes, push branches, or open fix PRs. Remediation belongs to a separate engineering workflow.

---

## Severity Levels & GitHub Label Convention

Every filed issue receives:

- One severity label: `severity:P0`, `severity:P1`, `severity:P2`, or `severity:P3`
- Standard triage labels: `regression`, `qa-triage`
- Slice ownership label when applicable: `slice:<id>` (e.g., `slice:A1`)

| Severity                                      | GitHub Label  | Label Color | Impact Definition                                                                                                                                       | Concrete Examples                                                                                                                                                                                                                                                                                                                                |
| --------------------------------------------- | ------------- | ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **P0 — Critical / Blocker**                   | `severity:P0` | `#b60205`   | Feature is completely broken, unusable, crashes, or breaks safety/rollback guarantees.                                                                  | - Black or blank screen where the migrated view should render<br>- App crash, freeze, or unhandled panic/WASM init failure<br>- Changing a preference does not persist to `preferences.json` or fails to broadcast to open Editor windows<br>- Instant rollback (`flag=legacy`) fails to restore the legacy view                                 |
| **P1 — High (Workaround Exists)**             | `severity:P1` | `#d93f0b`   | Core interaction or functionality is broken, causing significant friction, **but** the user can still complete the task via a workaround.               | - **Cannot scroll** the settings pane or panel (workaround: resize the window to reveal lower controls)<br>- Keyboard navigation or shortcut handling broken (mouse still works)<br>- Missing required setting control that is still editable via config file<br>- Severe performance/memory regression (>2× latency or unbounded memory growth) |
| **P2 — Medium-Low (UX / Control Divergence)** | `severity:P2` | `#fbca04`   | Feature works end-to-end and remains usable without blocking friction, but the UI control pattern or interaction model noticeably diverged from legacy. | - **Dropdown component changed to a list of buttons** (user can still pick every option)<br>- Missing helper description, validation hint, or tooltip<br>- Section ordering or grouping differs from legacy without losing functionality                                                                                                         |
| **P3 — Lowest (Cosmetic / Visual Polish)**    | `severity:P3` | `#0e8a16`   | Purely visual micro-differences with zero impact on usability or task completion.                                                                       | - **Button border-radius** differs from the legacy control<br>- **Font spacing, letter-spacing, kerning, or line-height** differs slightly<br>- Subtle padding, margin, divider weight, or color shade mismatch                                                                                                                                  |

---

## Resource Efficiency: Hourly Change-Gated Cron

To avoid wasting compute or Cursor Agent credits when nothing has merged:

1. **Trigger:** Runs every hour (`0 * * * *`) on `develop`, plus manual `workflow_dispatch` (with `force`, `slice`, `runtime`, and `dry_run` inputs).
2. **Job 1 (`check-changes`):** Lightweight git check (~3–5 seconds, no Node/Rust/Electron install) comparing `origin/develop` against the moving watermark tag `qa/last-audited`.
   - If `HEAD == qa/last-audited` (and `force != true`), **Job 2 is skipped completely** — zero VM or agent spin-up.
   - If commits changed only non-app files (e.g., unrelated docs outside app/migration paths) and `force != true`, Job 2 is also skipped and the watermark can stay clean.
3. **Job 2 (`qa-audit`):** Only runs when relevant changes exist (or `force == true`). Uses `@cursor/sdk` (`runtime=cloud` by default) to spin up a pre-warmed Cursor Cloud Agent VM from `.cursor/environment.json`, executes Before/After Playwright + `migration:measure` checks under Xvfb, inspects screenshots and PR diffs, uploads its structured findings as a run artifact.
4. **Job 3 (`file-findings`):** Downloads the saved report, files deduplicated GitHub issues, and advances `qa/last-audited` only after an automatic-scope audit covers every assigned slice and filing succeeds. A dry run or single-slice run records its own results and leaves the repository checkpoint in place.

---

## Recover findings after an issue-filing failure

A completed agent audit uploads `audit-report.json` as the `qa-audit-report-<run ID>` artifact **before** the filing job starts. If GitHub rejects an issue or label, fix the cause and select **Re-run failed jobs** on that workflow run. The filing job reuses the saved report, checks which findings already have open issues, and files the rest. The completed Cursor job stays in the original attempt. A successful filing also publishes `telemetry.json`.

To preview or replay outside Actions, download the report artifact from the run. From a checkout that contains the recovery command, preview the saved report using the repository and full commit SHA shown in the original audit. Replace the example placeholders with those values and the path to the downloaded file.

```sh
pnpm migration:qa-monitor --replay <path-to-audit-report.json> --repo <owner/repo> --head-sha <audited-sha> --dry-run
```

Run the same command without `--dry-run` to file the remaining findings. Replay uses your authenticated GitHub CLI, refreshes the list of open regression issues, and needs no Cursor API key. The repository checkpoint stays at the last successful automatic-scope workflow run.

## Deduplication Guarantee

Every filed issue embeds a stable HTML comment marker:

```html
<!-- qa-fingerprint: A1:scroll-broken-general-pane -->
```

Before creating an issue, the deterministic issue filer fetches all open `regression` issues and skips a finding when its normalized fingerprint already matches an open issue, including dotted slice IDs such as `W2.A`.
