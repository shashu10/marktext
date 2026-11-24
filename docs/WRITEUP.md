# Incremental Electron → Rust (GPUI) Migration with Cursor Cloud Agents & `@cursor/sdk`

> **Repo:** [`shashu10/marktext`](https://github.com/shashu10/marktext) (`develop`) · **One-command demo:** `pnpm migration:demo` ([README](../README.md#run-the-prototype-in-one-command))

## 1. The Problem & Who Has It

Enterprise desktop engineering teams maintaining large Electron applications (`100k+` LOC) face a recurring dilemma: Chromium's multi-process memory footprint (`475–650 MB` RSS), startup latency, and `270+ MB` installer size degrade user experience, yet a big-bang rewrite freezes product shipping for quarters and risks severe cross-OS regressions.

Using **MarkText** (`~125k LOC` Electron 45 + Vue 3 + Pinia + `@muyajs/core`) as a real enterprise proxy, this prototype demonstrates an end-to-end **Strangler-Fig migration to Rust (GPUI)** powered by **Cursor Cloud Agents (custom environment)** and **event-driven `@cursor/sdk` GitHub Actions workflows**—keeping legacy releases shippable with instant rollback at every step.

## 2. Architecture

**Empirical target selection (4-framework benchmark):** Before writing migration code, we built release prototypes of **GPUI (`0.2.2`)**, **Flutter (`3.50.1`)**, **Compose Multiplatform (`1.12.2`)**, and **Electron (`45.4.1`)** on identical Ubuntu 24.04 software-rendered runners across Hello World startup/memory and a 1,560-line Markdown 600-frame scroll + per-character edit workload. GPUI beat Electron on every resource axis—**15.8× smaller bundle** (`17.3` vs. `273.6 MiB`), **3.4× faster startup** (`51.8` vs. `173.7 ms`), **2.7× lower private dirty RAM** (`31.0` vs. `83.6 MiB`), and **`60 fps`** editing (`0.55 ms` paint, `+2 MiB` growth vs. Compose's `51 fps` and `+490 MiB` retained heap)—while `gpui-ce 0.3.0` also compiles cleanly to `wasm32-unknown-unknown`.

**Two-Stage Strangler Fig over `HostBridge`:** All Rust UI crates (`marktext-core` → `marktext-ui-kit` → leaf `mt-pref-*`) depend only on a transport-agnostic `HostBridge` trait with zero platform `#[cfg]` inside views:
- **Stage 1 (`rust-wasm`):** Compiles GPUI to WASM inside one persistent Electron `<GpuiPaneHost>` iframe (`WasmPostMessageBridge`), letting parallel Cloud Agents migrate all 8 Preferences slices (`A1`–`A8`) behind multi-environment feature flags (`packages/desktop/src/shared/flags/environments.json`, `prod` locked to `legacy`).
- **Stage 2 (`gpui-native`, `W2.A`):** Compiles the exact same 8 Rust crates unchanged into a native OS GPUI binary (`mt-pref-app`) spawned by Electron over NDJSON stdio (`NativeIpcBridge`) behind `window.preferences=gpui-native`, retiring Chromium's settings `BrowserWindow` while preserving Electron Main's `preferences.json` broadcast and instant rollback.

**Custom Cloud Agent VM & 3 decoupled `@cursor/sdk` CI workflows:**
- **Custom VM (`.cursor/`):** Ubuntu 24.04 image pre-warming Node 22, Electron, Rust, `wasm32`, `lld` (`CARGO_BUILD_JOBS=3`), Xvfb, Mesa Lavapipe Vulkan, `scrot`, `migration:build-wasm`, `migration:build-native-pref`, and `cargo test --no-run`, paired with `.cursor/skills/` and path-scoped `.cursor/rules/*.mdc`.
- **Three event-driven `@cursor/sdk` workflows** (each gated by a `~3s`, `$0` deterministic check): (1) **Hourly QA/TPM Monitor** (`migration-qa-monitor.yml`) launches a read-only Cloud Agent VM against `QA_RUBRIC.md` (`P0`–`P3`) and files fingerprint-deduplicated issues; (2) **Issue Triage** (`issue-triage.yml`) runs git forensics on human bugs and QA runs (via a zero-secret `workflow_run` bridge) and routes via `docs/migration/OWNERS.json`; (3) **Legacy-to-Rust Parity Sync** (`legacy-sync.yml`) detects drift when legacy Vue/TS edits touch a migrated slice and opens a forward-port PR (`autoCreatePR`).

## 3. Key Design Decisions & Trade-Offs

- **Strangler Fig vs. Big-Bang:** Lessons from 2026 Rust migrations (Bun, GitHub Copilot Runtime, Cap) showed big-bang rewrites require massive test oracles and halt shipping. Stage 1 WASM adds temporary coexistence RAM/WASM weight (`+115 MB` RSS, `+56.6 MB` `.wasm`), which disappears once a window flips to Stage 2 Native (`W2.A`).
- **Deterministic vs. agentic levers:** Rollout flags, LOC progress (`migration:progress`), RSS/latency collection (`migration:measure`), `~3s` change/drift gates, deduplication, and compare-and-swap watermarks are deterministic TypeScript; LLM spend is reserved strictly for code porting, visual/behavioral QA, git forensics, and forward-porting.
- **Compounding memory & governance:** Non-obvious runtime lessons (`size_full`, `READY`/`HOST_HELLO` handshake, `serde_wasm_bindgen` JS `Map`, `EditableTextState` overwrites) live in `docs/migration/MEMORY.md` and `.cursor/rules/rust-gpui-panes.mdc`. Agents cannot weaken tests, apply CI waivers, or self-merge.
- **Context boundaries:** Private planning/stakeholder materials stay in an outer wrapper workspace outside this public repo; Cloud Agents read only sanitized engineering contracts. In an enterprise, this maps directly to RBAC-scoped MCP/RAG connectors (Linear, Confluence, Datadog).

## 4. How We Know It Works

- **Coverage & parallel throughput:** **`99.56%` of Preferences (`5,595 / 5,620` legacy LOC; `11,605` Rust LOC)** merged across 24 PRs running upstream MarkText CI. After canary `A1` ([PR #5](https://github.com/shashu10/marktext/pull/5)) and `MEMORY.md` ([PR #6](https://github.com/shashu10/marktext/pull/6)), **6 parallel Cloud Agents (`A2`–`A7`, [PR #9](https://github.com/shashu10/marktext/pull/9)–[#14](https://github.com/shashu10/marktext/pull/14)) opened and merged in 39 minutes** (`12:51`–`13:30 UTC`) with zero collisions.
- **Stage 1 vs. Stage 2 scorecard (`W2.A`, [PR #21](https://github.com/shashu10/marktext/pull/21), Cloud Linux Xvfb + Lavapipe):**

| Metric | Legacy Vue | Stage 1 WASM (`A1`–`A8`) | Stage 2 Native (`W2.A`) | Rollback (`legacy`) |
| --- | ---: | ---: | ---: | ---: |
| Startup / Prefs Open (ms) | `1,601` / `451` | `820` / `1,581` | **`813`** / `1,516` *(SW Vulkan)* | `833` / `443` |
| Electron + Child RSS (MB) | `647.1` | `762.3` *(coexistence tax)* | **`456.7` + `170.4` = `627.1`** | `646.8` |

- **Live QA & container profiling:** Running `migration-qa-monitor.yml` live with `CURSOR_API_KEY` auto-filed **9 real `P1`/`P2` issues** ([`#25`–`#33`](https://github.com/shashu10/marktext/issues)), with dropdown parity ([Issue #26](https://github.com/shashu10/marktext/issues/26)) already fixed in [PR #34](https://github.com/shashu10/marktext/pull/34). A live Cloud VM audit caught **`cargo test` spiking to `7.91 GiB`** (GNU `ld` + third-party DWARF) while every other stage stayed `<2.1 GiB`; shipping `lld`, `CARGO_BUILD_JOBS=3`, and `[profile.dev.package."*"] debug = false` ([PR #23](https://github.com/shashu10/marktext/pull/23)) right-sizes VMs to `4–5 vCPU` / `8–12 GiB RAM` / `50–64 GiB disk`.

## 5. Where It Fails, Limitations & Scope

- **Linux Cloud VMs miss macOS/Windows bugs:** Local macOS verification caught 4 bugs invisible in Linux containers: SwiftShader breaking Metal WebGL2 ([PR #19](https://github.com/shashu10/marktext/pull/19)), dark-theme mismatch ([Issue #18](https://github.com/shashu10/marktext/issues/18)), `serde_wasm_bindgen` JS `Map` breaking sidebar clicks ([PR #20](https://github.com/shashu10/marktext/pull/20)), and a `gpui_ce_macos` startup mutex deadlock ([PR #22](https://github.com/shashu10/marktext/pull/22)).
- **Human steering & deterministic edges:** An early unconstrained 30-minute run expanded scope until reined in (motivating Draft-PR-first checkpoints). A live 36-minute QA run also lost its report when GitHub rejected a long label description—fixed in [PR #24](https://github.com/shashu10/marktext/pull/24) via Actions artifact replay (`$0` Cursor re-runs).
- **Open blockers & unmeasured metrics:** `prod` stays on `legacy` due to dark-theme sync ([Issue #18](https://github.com/shashu10/marktext/issues/18)), cold paint latency ([Issue #25](https://github.com/shashu10/marktext/issues/25)), Lavapipe `8.09%` idle CPU, and P2 parity items (`#27`–`#32`). Matched manual baselines, active maintainer review minutes, and billed cost per accepted PR were not recorded in this timebox.
- **Built vs. Design Only:** Preferences (`A1`–`A8` + `W2.A`), flags, harnesses, profiled VM, and live QA Monitor are **built & verified**; Issue Triage & Legacy-to-Rust Sync are **built & unit-tested** (`13dae1df`, awaiting production soak); Waves 1B–4 (`~119.5k` remaining LOC, recalibrated at **~4.5–6.5 months** for 1–2 engineers steering 4–6 agents), runtime `shadow` diffing, enforced branch protection/`CODEOWNERS`, and enterprise MCP/RAG connectors are **design only**.

## 6. With Another Week

1. **Unblock Preferences `prod` canary:** Fix dark-theme sync ([Issue #18](https://github.com/shashu10/marktext/issues/18)) and cold paint ([Issue #25](https://github.com/shashu10/marktext/issues/25)), merge [PR #24](https://github.com/shashu10/marktext/pull/24), and benchmark native `W2.A` idle CPU on macOS Metal and Windows D3D11 hardware.
2. **Run a matched Wave 1B pilot (`B1`–`B6`) with full ROI telemetry:** Measure active maintainer time (coding + review + rework), PR accept/reject/retry rates, escaped defects, and billed token+VM cost per accepted PR against a manual baseline.
3. **Harden enterprise governance & context:** Enforce branch protection + `CODEOWNERS` on `flags/` and `.cursor/`, add macOS/Windows CI smoke tests, soak-test `issue-triage.yml` + `legacy-sync.yml` end-to-end, and wire RBAC-scoped MCP tools (Linear/Confluence/telemetry).
