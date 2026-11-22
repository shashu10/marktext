# MarkText Electron → Rust (GPUI) Incremental Migration Plan

This RFC defines the **Strangler-Fig ("Ship of Theseus")** architecture and phased execution plan for incrementally migrating MarkText (`~104k LOC` Electron + Vue 3) to **Rust + GPUI** while keeping active legacy feature development and releases unblocked.

---

## 1. Core Migration Principles

1. **Incremental over Big-Bang:** Replace isolated leaf slices behind multi-environment feature flags (`legacy` | `rust-wasm` | `gpui-native` | `shadow`) so regressions map to a single PR and production releases stay uninterrupted.
2. **Write Every GPUI View Once, Ship Twice:** Use Zed's GPUI abstraction (`gpui-ce` + `gpui_ce_platform`) so every migrated UI component compiles to **both** `gpui_web` (WASM inside Electron during incremental rollout) and native OS GPUI (Metal / Vulkan / DirectX once a full window flips).
3. **Preferences Window First, Muya Editor Last:** `SettingWindow` is already an independent `BrowserWindow` with zero Muya coupling and a clean JSON-schema IPC contract. `@muyajs/core` is 62,272 LOC (~60% of the app) and is migrated last once the Rust core, bridge, theme/i18n tokens, and QA gates are battle-tested.
4. **Parallelism by Contract:** Once Phase 0 contracts and shared GPUI primitives exist, independent Cloud Agents can migrate leaf panes in parallel without file collisions.
5. **Translate First, Redesign Second:** Preserve legacy behavior, control semantics, and visual layout 1:1 during each slice port (validated by Bun and GitHub Copilot's Rust migrations). Never mix language translation with opportunistic architectural redesigns or UX changes; defer idiomatic refactors and deeper optimizations until after behavioral parity passes. Every slice must also state its 100% Rust end-state unambiguously so agents do not leave business logic or I/O stranded in Vue/TS.

---

## 2. Two-Stage Rendering & Co-Hosting Architecture

| Stage                                          | Rendering Target                                                                         | How It Runs                                                                                                                                                                                                                 | Scope                                                                                                                                                                 | Key Trade-Offs                                                                                                                                |
| ---------------------------------------------- | ---------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| **Stage 1: In-Window Incremental Replacement** | `gpui_web` (`wasm32-unknown-unknown`, WebGPU → WebGL2 fallback)                          | Mounted inside a borderless, persistent **`<iframe>`** (`<GpuiPaneHost>`) inside the existing Electron Vue window. Per-pane feature flags route individual tabs/panels to `legacy` (Vue) vs. `rust-wasm` (GPUI).            | Migrating the **7 Preferences panes in parallel**, plus isolated panels (**About**, **Document Statistics**, **Outline/TOC**) while keeping legacy Vue chrome intact. | Zero OS airspace bugs; works identically on macOS, Windows, and Linux/Xvfb Playwright CI. Temporary WASM size overhead disappears at Stage 2. |
| **Stage 2: Full Window Flip**                  | Native OS GPUI (`gpui_macos` Metal / `gpui_linux` Blade-Vulkan / `gpui_windows` DirectX) | The **exact same Rust GPUI view crates** compile natively into a sidecar binary. Flipping the window-level flag (`preferencesWindow: "gpui-native"`) launches the native GPUI window instead of Electron's `BrowserWindow`. | Retiring entire Electron windows (`SettingWindow` in Wave 2, `EditorWindow` in Wave 4) once all child panes pass parity.                                              | Eliminates Chromium + WASM overhead completely; native frame times and drastically lower RSS memory.                                          |

### Transport-Agnostic Bridge (`HostBridge`)

All Rust UI crates communicate with the host exclusively through a single trait in `rust/crates/marktext-core`:

```rust
pub trait HostBridge {
    fn get_preferences(&self) -> PreferencesSnapshot;
    fn set_preference(&self, key: String, value: serde_json::Value);
    fn subscribe_preferences(&self, on_patch: Box<dyn Fn(PreferencesPatch)>);
    // Main-window panel hooks (Wave 1B / Wave 2):
    fn scroll_to_header(&self, slug: String);
    fn toggle_reading_mode(&self, read_only: bool);
}
```

Only the thin transport adapter swaps as a window graduates stages:

1. **Stage 1 (`gpui_web` WASM in `<iframe>`):** `WasmPostMessageBridge` (`wasm-bindgen` ↔ `window.parent.postMessage` ↔ Vue `<GpuiPaneHost>` ↔ Electron `preload` IPC `mt::set-user-preference` / `mt::user-preference` / `bus.emit('scroll-to-header')`). Mutations immediately broadcast to all open Electron windows.
2. **Stage 2 (Native GPUI Window alongside Electron):** `NativeIpcBridge` (JSON-RPC over stdio / Unix socket directly to Electron Main's `Preference` service and `preferences.json`).
3. **Stage 3 (Pure Rust App, Electron Retired):** `DirectCoreBridge` (in-process calls directly to `marktext-core`).

### Non-Negotiable `gpui_web` & Native GPUI Engineering Constraints

1. **Single-threaded WASM on stable Rust:** Depend on `gpui-ce` and `gpui_ce_platform` with `default-features = false` (no nightly `build-std`, no `SharedArrayBuffer`, no COOP/COEP headers required).
2. **Explicit bundled fonts:** `gpui_web` has no OS system-font lookup (`CosmicTextSystem::new_without_system_fonts`); embed a UI font via `include_bytes!` and call `cx.text_system().add_fonts(...)` at startup.
3. **One window / one canvas per document:** `gpui_web` owns a single `<canvas>` per browser document (`WebWindowError` if a second window opens). Keep **one persistent `<iframe>`** in `<GpuiPaneHost>` and switch active panes via `postMessage({ type: 'SET_PANE', pane })`.
4. **Cloud Agent Cargo caching & acyclic crate graph:** `.cursor/install.sh` pre-warms `cargo build --target wasm32-unknown-unknown --release` into the Cloud Agent Build snapshot so incremental pane builds run in **~1.6s** instead of ~81s cold. Keep the workspace dependency graph strictly acyclic (`marktext-core` [UI-agnostic] → `marktext-ui-kit` → leaf `mt-*` crates → host crates) so parallel agents never introduce circular dependencies or degrade inner-loop build speed.
5. **Browser-equivalent primitives live once in `marktext-ui-kit`:** Leaving HTML/DOM means text inputs (selection, copy/paste, undo, dead keys, CJK/IME), scroll views, dropdowns, and Tab/Shift-Tab keyboard focus traversal are no longer free (Cap's GPUI text input alone required ~2,700 LOC of Rust). Build and verify these once in `marktext-ui-kit`, wire explicit window/pane focus order in every view, and avoid one-line CSS effects (backdrop blur, masks) that would require bespoke shaders unless intentionally designed.
6. **Native GPUI idle wake-ups & framework patches (Stage 2):** GPUI polls on display vsync by default (up to 120Hz on ProMotion displays); explicitly track 5s idle CPU % at Stage 2 and apply/upstream an idle-stop policy (e.g., pause redraw polling after ~100ms of inactivity, which cut Cap's idle wake-ups from 121/s → 1/s). Pin GPUI versions intentionally and budget for small OS-compatibility patches where needed.
7. **In-place user data compatibility:** Legacy and GPUI builds must read and write the exact same `preferences.json`, markdown files, and recovery checkpoints with zero user data migration.
8. **3-Layer QA Verification for `<canvas>` UI (Debug + Release):** Since GPUI paints to `<canvas>` rather than inner DOM nodes, verify migrated panes across **both debug and release builds** (catching `debug_assert!` erasure and release-mode serialization/bounds traps) via:
   - **Rust unit/state tests** (pane logic & schema parity),
   - **Bridge contract tests** (`postMessage` ↔ Electron Main `preferences.json` & multi-window broadcast),
   - **Playwright visual screenshot diff & keyboard/scroll interaction checks** (WebGL2 works in headless Chromium under `xvfb-run` in Linux Cloud Agent VMs).

### Feasibility Baseline (Verified on Stable `rustc 1.100.0`, `gpui-ce 0.3.0`)

| Target                                 | Backend                                           | Cold Build (`--release`) | Incremental Rebuild |     Artifact Size (unstripped) |
| -------------------------------------- | ------------------------------------------------- | -----------------------: | ------------------: | -----------------------------: |
| **`wasm32-unknown-unknown`** (Stage 1) | `gpui_ce_web` + `gpui_ce_wgpu` + `gpui_ce_parley` |                   1m 21s |               1.59s | 17 MB `.wasm` (pre-`wasm-opt`) |
| **`aarch64-apple-darwin`** (Stage 2)   | `gpui_ce_macos` (Metal) + `gpui_ce_apple`         |                   1m 20s |               0.95s |            18 MB native binary |

---

## 3. Dependency DAG & Execution Waves

```
Phase 0: Shared Foundations (Sequential — Unblocks All Parallel Cloud Agents)
  ├── P0.1 Feature-Flag Router (`flags/environments.json`: legacy | rust-wasm | gpui-native | shadow)
  ├── P0.2 Rust Workspace (`rust/`): `marktext-core` (HostBridge, prefs schema, theme/i18n tokens)
  │        + `marktext-ui-kit` (GPUI primitives: Toggle, Select, Slider, TextInput, SettingRow, Modal, Tree)
  │        + `marktext-wasm-host` (`gpui_web` single-canvas router + `WasmPostMessageBridge`)
  ├── P0.3 Electron `<GpuiPaneHost>` persistent iframe + IPC bridge
  └── P0.4 Deterministic Parity, Perf/Memory & Visual QA Harness
        │
        ├─────────────────────────────────────────────────────────────┐
        ▼                                                             ▼
Wave 1A: Preferences Panes (All 8 in PARALLEL)                 Wave 1B: Isolated Canaries & Panels (All 6 in PARALLEL)
  ├── A1. General Pane                                           ├── B1. About Dialog (Zero-state canary)
  ├── A2. Appearance & Theme Pane                                ├── B2. Document Statistics Popover (counts + reading mode + stat)
  ├── A3. Editor & Spellchecker Pane                             ├── B3. Outline / TOC Panel (WYSIWYG + Source mode parity)
  ├── A4. Files & Startup/Recovery Pane                          ├── B4. Rename & Import Dialogs
  ├── A5. Markdown & Diagrams Pane                               ├── B5. Export Settings Dialog
  ├── A6. Images & Uploader Pane                                 └── B6. Command Palette
  ├── A7. Keybindings & Shortcut Recorder Pane
  └── A8. Preferences Sidebar & Cross-Category Search
        │                                                             │
        ▼ (GATE: All A1–A8 pass parity + live Main sync)              ▼
Wave 2A: Flip Entire Preferences Window to Native GPUI         Wave 2B: Editor Window Chrome & Navigation (Parallel)
  └── Retire `SettingWindow` BrowserWindow behind flag           ├── C1. Title Bar (Breadcrumb, Save Dot, Controls)
                                                                 ├── C2. Tab Bar (Reorder, Dirty/Preview, Close)
                                                                 ├── C3. Left Sidebar — File Tree & Opened Files
                                                                 ├── C4. Left Sidebar — Ripgrep Global Search
                                                                 ├── C5. In-Document Find & Replace Bar
                                                                 └── C6. Recent / Welcome Screen, MediaViewer, Notifications
                                                                      │
        ┌─────────────────────────────────────────────────────────────┘
        ▼
Wave 3: Core Backend Services & Dual Editor Engines
  ├── D1. Main Process Services → `marktext-core` (Watcher, Atomic File I/O, Encoding, Checkpoints/Sessions, Native Menus)
  ├── D2. Source Code Mode → Native GPUI Code Editor (replaces CodeMirror 5)
  └── D3. WYSIWYG Engine → Port `@muyajs/core` (62k LOC) in sequential sub-slices:
          ├── D3a. Markdown Parser / AST & Inline Engine (golden oracle: existing Muya CommonMark/GFM Vitest specs)
          ├── D3b. Block Layout, Typography, Heading Folding & Cursor/Selection/Undo
          ├── D3c. Tables, Code Blocks, Math (KaTeX), Diagrams (Mermaid/PlantUML)
          └── D3d. Image Paste/Drop/Upload, Export (HTML/PDF), Spellcheck & Reading/Focus/Typewriter Modes
        │
        ▼ (GATE: All B + C + D pass parity, perf, memory & visual regression)
Wave 4: Flip Entire Main Editor Window & Retire Electron Runtime
  └── Ship standalone pure-Rust GPUI MarkText binary
```

### Rollout & Parallelization Guardrails (Waves 1–4)

- **Canary Validation Before Fan-Out:** Prove Phase 0 scaffolding, `HostBridge`, and visual/perf baselines end-to-end on canary slices (`A1` General Pane / `B1` About Dialog) before launching parallel Wave 1A/1B agents.
- **Shadowing (Waves 1–2) vs. Atomic Leaf-Inward Cutover (Wave 3):** Per-pane feature flags and `shadow` mode work well for isolated UI panes with clean `HostBridge` boundaries (Waves 1–2). For deeply coupled, mutable stateful hubs in Wave 3 (`D1` session/checkpoint orchestration and `D3` Muya mutable state), avoid maintaining long-lived dual copies that drift under concurrent edits — instead port **leaf-inward in atomic waves** (_pure logic → state ownership → orchestration → remove shims → simplify_) and **never schedule simultaneous top-down and bottom-up parallel agents colliding on a shared hub**.
- **Immutable Wave 3 Conformance Oracle & Fuzzing:** Treat Muya's existing language-independent CommonMark + GFM Vitest conformance suite as an immutable parity oracle (`0` tests skipped or deleted). Once `D3a` lands in Rust, enable coverage-guided parser fuzzing on Markdown/inline parsers.
- **Wave 4 Electron-Retirement CI Guardrail:** Once Electron is retired in Wave 4, add a release CI assertion that fails the build if `electron`, Chromium, or webview dependencies re-enter the packaged artifact.

---

## 4. Enterprise Governance, Review & Measurement

- **Multi-Environment Feature Flags (`dev` / `staging` / `prod`):**
  - `dev`: Migrated slices active (`rust-wasm` / `gpui-native`) for rapid verification.
  - `staging`: `shadow` mode where applicable for isolated stateless/UI slices (runs legacy + Rust side-by-side, serves legacy, logs diffs & latency).
  - `prod`: Staged canary per slice with instant rollback to `legacy`.
- **Branch Protection, Split-Context Adversarial QA & Oracle Protection:**
  - Cloud Agents and automated workflows never push to `develop` or self-merge.
  - **Protect the Oracle:** Implementing agents must never delete, skip, or weaken existing unit/E2E/conformance tests, stub out failing code to appease the compiler, relax visual-diff thresholds, or apply CI waiver/override labels.
  - **Split-Context Independent Verification:** Migration PRs target `develop` with label `migration` → trigger deterministic CI (`lint`, `typecheck`, unit tests, Rust debug + release tests, `clippy`, `wasm32` build) + the independent `@cursor/sdk` QA Monitor running in a **fresh context window** (inspecting diffs, metrics, and screenshots without implementer bias) → require green checks + human `CODEOWNERS` LGTM.
- **Metrics Tracked per Rollout:**
  - **Delivery:** Cycle time per migrated slice (before vs. after), PRs merged vs. blocked/rejected, failure & retry rates, cost per agent run (and prompt-cache efficiency).
  - **Runtime:** Startup latency, RSS memory per window, idle CPU % (including native vsync wake-ups), bundle/binary size, large-document parse/render/scroll p95 latency, visual pixel-diff %.
- **Execution Runbook:** Every Cloud Agent must follow the Draft-PR-first Before/After measurement and human-review protocol in [`docs/migration/RUNBOOK.md`](./RUNBOOK.md).
