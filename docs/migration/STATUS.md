# Migration Slice Status Tracker

> Single source of truth for Cloud Agents and human reviewers coordinating parallel workstreams.
> Always claim/update your slice row in your PR so parallel agents never collide on the same target crate or feature flag.

**Legend:** `todo` · `blocked` (waiting on prereqs) · `in-progress` · `in-review` · `verified`

---

## Phase 0 — Shared Foundations (Sequential)

| Slice ID | Component / Deliverable                                                   | Feature Flag Key | Prerequisites  | Target Paths                                             | Status |
| -------- | ------------------------------------------------------------------------- | ---------------- | -------------- | -------------------------------------------------------- | ------ |
| `P0.1`   | Multi-env Feature Flag Router (`dev` / `staging` / `prod`)                | `flags/*`        | —              | `packages/desktop/src/shared/flags/`                     | `todo` |
| `P0.2`   | Rust Workspace (`marktext-core`, `marktext-ui-kit`, `marktext-wasm-host`) | —                | —              | `rust/`                                                  | `todo` |
| `P0.3`   | Electron `<GpuiPaneHost>` Persistent Iframe & `WasmPostMessageBridge`     | —                | `P0.1`, `P0.2` | `packages/desktop/src/renderer/src/components/gpuiHost/` | `todo` |
| `P0.4`   | Deterministic Parity, Perf/Memory & Visual QA Harness                     | —                | `P0.3`         | `scripts/migration-qa/`, `.github/workflows/`            | `todo` |

---

## Wave 1A — Preferences Window Panes (Parallel Cloud Agents)

| Slice ID | Preference Pane                     | Feature Flag Key         | Prerequisites | Legacy Source                                            | Target Rust Crate                 | Status    |
| -------- | ----------------------------------- | ------------------------ | ------------- | -------------------------------------------------------- | --------------------------------- | --------- |
| `A1`     | General                             | `prefs.pane.general`     | `P0.3`        | `prefComponents/general/`                                | `rust/crates/mt-pref-general`     | `blocked` |
| `A2`     | Appearance & Theme                  | `prefs.pane.appearance`  | `P0.3`        | `prefComponents/theme/`, `general`, `editor`             | `rust/crates/mt-pref-appearance`  | `blocked` |
| `A3`     | Editor & Spellchecker               | `prefs.pane.editor`      | `P0.3`        | `prefComponents/editor/`, `spellchecker/`                | `rust/crates/mt-pref-editor`      | `blocked` |
| `A4`     | Files & Startup/Recovery            | `prefs.pane.files`       | `P0.3`        | `prefComponents/general/recoverySettings.vue`, `editor/` | `rust/crates/mt-pref-files`       | `blocked` |
| `A5`     | Markdown & Diagrams                 | `prefs.pane.markdown`    | `P0.3`        | `prefComponents/markdown/`                               | `rust/crates/mt-pref-markdown`    | `blocked` |
| `A6`     | Images & Uploader                   | `prefs.pane.images`      | `P0.3`        | `prefComponents/image/`                                  | `rust/crates/mt-pref-images`      | `blocked` |
| `A7`     | Keybindings & Recorder              | `prefs.pane.keybindings` | `P0.3`        | `prefComponents/keybindings/`                            | `rust/crates/mt-pref-keybindings` | `blocked` |
| `A8`     | Preferences Sidebar & Global Search | `prefs.sidebar`          | `P0.3`        | `prefComponents/sideBar/`, `registry.ts`                 | `rust/crates/mt-pref-shell`       | `blocked` |

---

## Wave 1B — Isolated Dialogs & Editor Panels (Parallel Cloud Agents)

| Slice ID | Component                   | Feature Flag Key        | Prerequisites | Legacy Source                                              | Target Rust Crate                | Status    |
| -------- | --------------------------- | ----------------------- | ------------- | ---------------------------------------------------------- | -------------------------------- | --------- |
| `B1`     | About Dialog _(Canary #1)_  | `dialog.about`          | `P0.3`        | `components/about/`                                        | `rust/crates/mt-dialog-about`    | `blocked` |
| `B2`     | Document Statistics Popover | `panel.docStats`        | `P0.3`        | `components/titleBar/documentStatistics.vue`               | `rust/crates/mt-panel-doc-stats` | `blocked` |
| `B3`     | Outline / TOC Panel         | `panel.outlineToc`      | `P0.3`        | `components/editorWithTabs/outline.vue`, `sideBar/toc.vue` | `rust/crates/mt-panel-toc`       | `blocked` |
| `B4`     | Rename & Import Dialogs     | `dialog.renameImport`   | `P0.3`        | `components/rename/`, `components/import/`                 | `rust/crates/mt-dialog-file-ops` | `blocked` |
| `B5`     | Export Settings Dialog      | `dialog.exportSettings` | `P0.3`        | `components/exportSettings/`                               | `rust/crates/mt-dialog-export`   | `blocked` |
| `B6`     | Command Palette             | `dialog.commandPalette` | `P0.3`        | `components/commandPalette/`                               | `rust/crates/mt-command-palette` | `blocked` |

---

## Wave 2 — Preferences Window Native Flip & Main Editor Chrome

| Slice ID | Component / Milestone                         | Feature Flag Key     | Prerequisites | Target                                        | Status    |
| -------- | --------------------------------------------- | -------------------- | ------------- | --------------------------------------------- | --------- |
| `W2.A`   | **Native GPUI Preferences Window Flip**       | `window.preferences` | `A1`–`A8`     | `rust/crates/mt-pref-app` + `NativeIpcBridge` | `blocked` |
| `C1`     | Title Bar (Breadcrumb, Save Dot, Controls)    | `chrome.titleBar`    | `P0.3`        | `rust/crates/mt-chrome-titlebar`              | `blocked` |
| `C2`     | Tab Bar (Reorder, Dirty/Preview, Close)       | `chrome.tabBar`      | `P0.3`        | `rust/crates/mt-chrome-tabs`                  | `blocked` |
| `C3`     | Left Sidebar — File Tree & Opened Files       | `sidebar.files`      | `P0.3`        | `rust/crates/mt-sidebar-files`                | `blocked` |
| `C4`     | Left Sidebar — Ripgrep Global Search          | `sidebar.search`     | `P0.3`        | `rust/crates/mt-sidebar-search`               | `blocked` |
| `C5`     | In-Document Find & Replace Bar                | `panel.findReplace`  | `P0.3`        | `rust/crates/mt-panel-find`                   | `blocked` |
| `C6`     | Recent Screen, MediaViewer, Tab Notifications | `chrome.misc`        | `P0.3`        | `rust/crates/mt-chrome-misc`                  | `blocked` |

---

## Wave 3 & Wave 4 — Core Services, Dual Editors & Main Window Flip

| Slice ID | Component / Milestone                                       | Feature Flag Key     | Prerequisites               | Notes                                                         | Status    |
| -------- | ----------------------------------------------------------- | -------------------- | --------------------------- | ------------------------------------------------------------- | --------- |
| `D1`     | Main Process Services → `marktext-core`                     | `core.services`      | `P0.2`                      | Watcher, atomic writes, encoding, sessions/checkpoints, menus | `blocked` |
| `D2`     | Source Code Mode (replaces CodeMirror 5)                    | `editor.sourceCode`  | `W2`, `D1`                  | Native GPUI text/code editor                                  | `blocked` |
| `D3a`    | Muya Parser / AST & Inline Engine                           | `editor.muya.parser` | `D1`                        | Verified against existing CommonMark/GFM Vitest specs         | `blocked` |
| `D3b`    | Muya Block Layout, Folding & Selection/Undo                 | `editor.muya.layout` | `D3a`                       | Core interactive WYSIWYG loop                                 | `blocked` |
| `D3c`    | Muya Tables, Code Blocks, Math & Diagrams                   | `editor.muya.rich`   | `D3b`                       | KaTeX, Mermaid, Flowchart, PlantUML                           | `blocked` |
| `D3d`    | Muya Images, Export, Spellcheck & View Modes                | `editor.muya.polish` | `D3c`                       | Focus, typewriter, reading mode, PDF/HTML export              | `blocked` |
| `W4`     | **Full Native GPUI Main Window Flip & Electron Retirement** | `window.main`        | `W2`, `C1`–`C6`, `D1`–`D3d` | Pure-Rust GPUI MarkText binary                                | `blocked` |
