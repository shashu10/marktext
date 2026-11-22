# MarkText (`v0.24.0-dev`) — Architecture & Feature Inventory

This document maps the existing Electron + Vue 3 architecture, subsystem sizes, IPC contracts, and critical user interaction flows to ground the incremental Electron → Rust (GPUI) migration.

---

## 1. Current Stack & Baseline Footprint

- **Stack:** pnpm monorepo (`pnpm 10`, Node `>=22.12`, TypeScript 6 strict, Electron `~45.0.0`, `electron-vite`, Vue 3.5, Pinia 4, Element Plus 2, CodeMirror 5, Vitest, Playwright).
- **Renderer Bundle Baseline (`pnpm build:unpack`):**
  - Primary renderer chunk (`out/renderer/assets/index-*.js`): **7.02 MB**
  - Heavy lazy chunks: `elk` (**2.62 MB**), `embed` (**1.62 MB**), `embedExportFonts` (**916 KB**), `cytoscape` (**869 KB**), `katex` (**499 KB**), `codemirror` (**322 KB**), `architectureDiagram` (**318 KB**), plus Chromium baseline RSS per window.

### Non-Test Source Breakdown

| Subsystem                     | Path                                                |   Files |       Lines | Role                                                                                                                                |
| ----------------------------- | --------------------------------------------------- | ------: | ----------: | ----------------------------------------------------------------------------------------------------------------------------------- |
| **Muya Editor Core**          | `packages/muya/src`                                 |     327 |      62,272 | Custom WYSIWYG Markdown parser, block tree, inline engine, selection/undo, diagrams                                                 |
| **Electron Main Process**     | `packages/desktop/src/main`                         |     100 |      15,143 | Windows, file I/O, watcher, prefs persistence, menus, keybindings, session/checkpoints, IPC, ripgrep, Pandoc                        |
| **Editor UI Components**      | `packages/desktop/src/renderer/src/components`      |      43 |      11,352 | Title bar, Document Statistics, Sidebar (Files/Search), Outline/TOC, Tabs, Editor, Source Code, Dialogs                             |
| **Preferences UI**            | `packages/desktop/src/renderer/src/prefComponents`  |      50 |       5,296 | 7 preference categories, shared form controls, live search, keybinding recorder                                                     |
| **Pinia Stores**              | `packages/desktop/src/renderer/src/store`           |      13 |       3,646 | Reactive renderer state & IPC listeners (`editor`, `preferences`, `layout`, `project`, `commandCenter`, etc.)                       |
| **Renderer Utils & Services** | `packages/desktop/src/renderer/src/{util,services}` |      35 |       3,546 | TOC navigation, active heading lookup, export HTML/PDF, themes, session/view state                                                  |
| **Shared Contracts**          | `packages/desktop/src/shared`                       |      14 |       2,174 | Strictly typed IPC channels, preferences JSON schema, session/file/menu types                                                       |
| **Sandboxed Preload Bridge**  | `packages/desktop/src/preload/index.ts`             |       1 |         302 | `contextBridge` APIs (`window.electron`, `window.fileUtils`, `window.ripgrep`, `window.fonts`, `window.uploader`, `window.diagram`) |
| **Total Core App**            |                                                     | **583** | **103,731** |                                                                                                                                     |

---

## 2. Reference Flow Trace: Outline (TOC) Heading Click → Editor Scroll

Understanding how decoupled UI components communicate with the editor illustrates the cleanest migration seams:

1. **TOC Extraction (`packages/muya/src/state/getTOC.ts`):** Muya walks only **top-level** heading blocks (`atx-heading` and `setext-heading` direct children of `scrollPage`) in document order, assigning each an in-memory per-block `slug` (cached in a `WeakMap`, not written as a DOM `id`), plus plain text, sanitized inline HTML, and `githubSlug`.
2. **Store Projection (`store/editor.ts`):** `editor.vue` calls `editorStore.REFRESH_TOC(muya.getTOC())`, storing both flat `listToc` and nested `toc` (`listToTree`).
3. **Click Emission (`components/sideBar/toc.vue`, wrapped by `components/editorWithTabs/outline.vue`):** Clicking a heading sets `preserveOutlinePosition = true` (preventing intermediate animation scroll events from moving the outline scroll position) and emits **`bus.emit('scroll-to-header', data.slug)`**.
4. **Dual-Mode Scroll Listeners:**
   - **WYSIWYG Mode (`components/editorWithTabs/editor.vue` + `util/tocNavigation.ts`):** Resolves `slug` to its 0-based index in `editorStore.listToc`, selects `headings[index]` from `.mu-container > h1..h6`, calls `editor.value?.revealElement(heading)` (`muya.ts` — automatically unfolds collapsed parent headings), computes `container.scrollTop + headingTop - containerTop - 24`, sets `revealingAnchor = true`, and runs `animatedScrollTo(container, top, 300)` (`util/index.ts`).
   - **Source Code Mode (`components/editorWithTabs/sourceCode.vue` + `util/sourceModeToc.ts`):** Maps `slug` → heading index → 0-based line number via `findMarkdownHeadingLine` (skipping fenced code blocks, matching ATX and setext), updates CodeMirror cursor with `{ scroll: false }`, and smoothly scrolls `.source-code` to `cm.heightAtLine(line, 'local')`.
5. **Active-Heading Feedback (`util/findActiveHeading.ts`):** On scroll/caret move, binary search over cached top-level heading DOM positions updates `editorStore.SET_ACTIVE_HEADING(slug)`, highlighting the active node (or nearest visible collapsed ancestor) in the outline.

**Seam Takeaway:** The TOC UI only consumes `toc`, `activeHeadingSlug`, and `wordWrapInToc` and emits a single command (`scroll-to-header(slug)`). All DOM measurement, unfolding, and scrolling stay behind the editor boundary.

---

## 3. Window & Feature Inventory

### A. Multi-Window Architecture (`packages/desktop/src/main/windows/`)

Two independent Electron `BrowserWindow` types routed via Vue Router (`?type=editor` vs. `?type=settings/<category>`):

1. **Editor Window (`EditorWindow` → `/editor` → `pages/app.vue`):** Multi-instance; owns workspace roots, tabs, session snapshots, and crash-recovery checkpoints.
2. **Preferences Window (`SettingWindow` → `/preference/:category` → `pages/preference.vue`):** Singleton; zero Muya coupling; talks to Main Process solely via IPC (`mt::ask-for-user-preference`, `mt::set-user-preference`, `mt::user-preference`) backed by `shared/preferences/schema.json` and `main/preferences/index.ts`.

### B. Main Editor Window UI Surfaces (`pages/app.vue` & `components/`)

| Surface                           | Component Path                                              | Key Features & Couplings                                                                                                                                                                                                                   |
| --------------------------------- | ----------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Title Bar**                     | `components/titleBar/index.vue`                             | Drag region, RTL-safe path breadcrumb, dirty dot, macOS click-to-rename, Win/Linux window controls & menu trigger.                                                                                                                         |
| **Document Statistics Popover**   | `components/titleBar/documentStatistics.vue`                | Cycles word/paragraph/char/char-with-space counts (incl. selection counts), **Reading Mode (read-only) toggle** (`SET_READ_ONLY` → `mt::update-document-mode`), and live file `created`/`modified` timestamps (`window.fileUtils.stat`).   |
| **Left Sidebar (Files & Search)** | `components/sideBar/`                                       | Resizable; switches between **Files** (Opened Tabs + multi-root workspace tree, drag-move, rename/trash, exclusion rules) and **Global Search** (streaming `ripgrep` IPC). Footer opens Settings & Add Folder.                             |
| **Document Outline / TOC**        | `components/editorWithTabs/outline.vue` + `sideBar/toc.vue` | Dockable left/right or hover-floating outline; resizable; expand/collapse all; per-tab collapsed state; active-heading highlight; emits `scroll-to-header`.                                                                                |
| **Tab Bar**                       | `components/editorWithTabs/tabs.vue`                        | Multi-tab strip, drag reorder, dirty/preview state, close/context menu.                                                                                                                                                                    |
| **WYSIWYG Editor**                | `components/editorWithTabs/editor.vue` + `@muyajs/core`     | CommonMark + GFM + math (KaTeX), Mermaid/Flowchart/Sequence/PlantUML, GitHub alerts, tables, task lists, heading folding, frontmatter, inline diff/highlight/sub/sup, image drop/paste/upload, spellcheck, focus/typewriter/reading modes. |
| **Source Code Editor**            | `components/editorWithTabs/sourceCode.vue`                  | CodeMirror 5 Markdown+Math raw editor, line numbers, undo history sync, shared TOC/search integration.                                                                                                                                     |
| **In-Document Find & Replace**    | `components/search/index.vue`                               | Case/whole-word/regex find & replace across WYSIWYG and Source modes.                                                                                                                                                                      |
| **Command Palette**               | `components/commandPalette/index.vue`                       | Fuzzy action launcher wired to `store/commandCenter.ts`.                                                                                                                                                                                   |
| **Export Settings Dialog**        | `components/exportSettings/index.vue`                       | HTML / PDF / Print export configuration modal.                                                                                                                                                                                             |
| **Import Modal**                  | `components/import/index.vue`                               | Drag-and-drop Pandoc import (`.docx`, `.tex`, `.html`, etc.).                                                                                                                                                                              |
| **Rename Dialog**                 | `components/rename/index.vue`                               | File rename dialog.                                                                                                                                                                                                                        |
| **About Dialog**                  | `components/about/index.vue`                                | Static version, credits, and license modal (zero-state migration canary).                                                                                                                                                                  |
| **Recent / Empty State**          | `components/recent/index.vue`                               | Welcome screen when no tab is open.                                                                                                                                                                                                        |
| **Media Viewer**                  | `components/mediaViewer/index.vue`                          | Zoom/pan lightbox for embedded images.                                                                                                                                                                                                     |
| **Tab Notifications**             | `components/editorWithTabs/notifications.vue`               | Non-blocking banners (external edits, encoding warnings, crash recovery).                                                                                                                                                                  |

### C. Preferences Window UI Surfaces (`pages/preference.vue` & `prefComponents/`)

- **Sidebar & Global Search (`prefComponents/sideBar/`, `registry.ts`, `settingLocation.ts`):** 7 category tabs plus live cross-category search across all settings & keybindings with deep-link highlight (`.is-search-target`).
- **Shared Controls (`prefComponents/common/`):** `bool`, `select`, `range`, `textBox`, `fontTextBox` (system fonts via IPC), `separator`, `compound`, `settingRow`, `settingNote`, `commandDetection`, `helpLink`.
- **7 Categories (`prefComponents/categories.ts` & `router/index.ts`):**
  1. `general` — Language, default open mode, window opening, sidebar/TOC behavior, tree exclude patterns.
  2. `appearance` — Theme picker + live previews, follow system theme, custom CSS, window chrome/zoom, typography.
  3. `editor` — Writing behavior (auto-pair, tab size, RTL/LTR, quick insert, link popup, task cascade), code blocks, and Spellchecker.
  4. `files` — Auto-save, startup & session/layout/view restoration, line endings, encoding detection, trailing newlines.
  5. `markdown` — Lists, headings, frontmatter, GFM/extensions (CJK emphasis, super/sub, footnotes, math, alerts, etc.), diagram themes & PlantUML server.
  6. `images` — Insert action, global & relative folders, uploaders (PicGo, GitHub, SM.MS, CLI script), Pandoc toggle.
  7. `keybindings` — Grouped shortcut list, conflict detection, interactive shortcut recorder, reset/autosave.

_(Note: `CategoryPage` currently composes subsets of `general/index.vue` and `editor/index.vue` across `general`, `appearance`, `editor`, `files`, and `images`; migrated Rust panes should bind directly to `shared/preferences/schema.json` and `prefComponents/registry.ts` metadata.)_

### D. Electron Main Process Services (`packages/desktop/src/main/`)

- **Preferences & Credentials:** Atomic `preferences.json` persistence + `broadcast-preferences-changed` to all open windows; `keytar` uploader secrets.
- **Filesystem & Watcher:** `chokidar` watcher, atomic document writes (`write-file-atomic`), encoding detection (`ced` native C++), trash, sidebar drag-move.
- **Session & Recovery:** `sessionHost`, `checkpointService`, `documentRegistry`, `windowGeometry`, `closeCoordinator`.
- **OS & External Tool Bridges:** Native application/context menus, `native-keymap`, native dark/light theme, CLI parser, auto-updater, Electron spellchecker, `ripgrep`, `pandoc`, `font-list`, PlantUML proxy, clipboard images.
