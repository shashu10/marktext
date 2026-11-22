# Development Rules

This file provides guidance to AI agents when working with code in this repository.

## Project Overview

MarkText is a WYSIWYG markdown editor built on Electron + Vue 3. It supports CommonMark, GitHub Flavored Markdown, math (KaTeX), Mermaid diagrams, PlantUML, and multiple editing modes (focus, typewriter, source-code).

- **Version**: see `package.json`
- **License**: MIT
- **Repository**: <https://github.com/marktext/marktext>

## Tech Stack

| Layer              | Technology                                                 |
| ------------------ | ---------------------------------------------------------- |
| Language           | TypeScript 6.0 (strict mode)                               |
| Desktop shell      | Electron 45                                                |
| Build system       | electron-vite 6                                            |
| Packaging          | electron-builder 26                                        |
| Frontend framework | Vue 3                                                      |
| State management   | Pinia 4                                                    |
| Routing            | Vue Router 5                                               |
| UI library         | Element Plus                                               |
| Unit tests         | Vitest 5                                                   |
| E2E tests          | Playwright                                                 |
| Package manager    | pnpm >=10 workspace (`packageManager: pnpm@10.33.4`)       |
| Repo layout        | pnpm monorepo — see Directory Structure                    |
| Node.js minimum    | >=22.12.0 (PR CI: Node 22.21.1 · release CI: Node 24.14.1) |

## Directory Structure

This is a pnpm workspace. Three packages live under `packages/`, and the
root holds only shared tooling and CI-facing scripts.

```
<repo-root>/
  package.json              Workspace orchestrator — desktop commands proxy to
                            packages/desktop via `pnpm --filter marktext`;
                            lint, formatting and maintenance run at root.
  pnpm-workspace.yaml       Workspace packages, including Muya examples/E2E,
                            dependency overrides and allowBuilds.
  pnpm-lock.yaml            Single lockfile, shared across all packages.
  eslint.config.js          Root ESLint 10 flat config (desktop and shared
                            tooling; Muya and website self-lint with their
                            own flat configs and are ignored here).
  scripts/                  Workspace-level scripts. postinstall.ts,
                            minify-locales.ts, generateThirdPartyLicense.ts,
                            validateLicenses.ts, thirdPartyChecker.ts all
                            target packages/desktop internally.
  docs/                     README translations and repository assets.
  dist/                     Packaged installers from electron-builder
                            (git-ignored; electron-builder writes here via
                            `directories.output: ../../dist` so CI artifact
                            globs `dist/*` still apply).
  packages/
    desktop/                The Electron app (name: "marktext").
      package.json          Holds all Electron / Vue / build-time deps and
                            the dev/build/test/typecheck scripts. Depends on
                            @muyajs/core via workspace:*.
      electron.vite.config.ts
      electron-builder.yml  directories.output points at ../../dist.
      tsconfig.json / tsconfig.base.json
      vitest.config.ts
      playwright.config.ts  Must stay here, not in test/e2e/ — Playwright only
                            auto-loads a config from the directory it runs in.
      patches/              pnpm patches consumed by patch-package.
      build/                electron-builder resources (icons, entitlements,
                            NSIS scripts).
      static/               Static assets bundled into the app
                            (icons, themes, locales).
      out/                  electron-vite output (git-ignored).
      test/
        unit/               Vitest specs → pnpm test / pnpm test:unit
        e2e/                Playwright specs → pnpm test:e2e
      src/
        common/             Pure Node.js utilities usable from main, preload,
                            and renderer.
        main/               Electron main process (IO, native dialogs, window
                            management, auto-updater).
        preload/            Electron preload scripts. The renderer runs
                            sandboxed (contextIsolation: true,
                            nodeIntegration: false, sandbox: true since
                            #4244) — all Node access flows through the typed
                            contextBridge surface in
                            packages/desktop/src/preload/index.ts.
        renderer/           Vue 3 application (editor UI, Pinia stores).
          src/
            components/     Vue single-file components.
            store/          Pinia stores (editor.ts, preferences.ts,
                            layout.ts, …).
            pages/          Top-level Vue pages / routes.
            router/         Vue Router configuration.
        shared/             Cross-process types (`shared/types/`) and the
                            IPC contract (`shared/types/ipc.ts`).
        types/              Ambient .d.ts declarations.
    muya/                   TypeScript rewrite of muya
                            (name: "@muyajs/core"; upstream:
                            https://github.com/marktext/muya). Built on
                            ot-json1 + ot-text-unicode + snabbdom + marked
                            + rxjs. Self-contained: own eslint config
                            (antfu), own stylelint, own madge, own vitest
                            spec suites (CommonMark + GFM). The editor
                            engine the desktop renderer consumes. See
                            packages/muya/AGENTS.md for layout and commands.
      src/                  TS source. Public entrypoint src/index.ts.
      test/spec/            CommonMark 0.31 + GFM 0.29-gfm conformance.
      examples/             muya-examples — vite vanilla-TS dev demo
                            (listed in pnpm-workspace.yaml).
      e2e/                  muya-e2e — Playwright suite. CI runs Chromium
                            only via muya-e2e.yml; Firefox + WebKit are
                            available locally with exclusions documented
                            in e2e/README.md and playwright.config.ts.
    website/                marktext-website (Next.js 15 + React 19).
                            Own lint/typecheck scripts and website-deploy CI;
                            OpenNext + Cloudflare Workers hosting.
      content/docs/         User and developer Markdown documentation.
      public/docs/          Documentation images and theme screenshots.
      src/ / scripts/ / next.config.mjs / wrangler.toml / tsconfig.json
```

The root has no `src/`, `test/`, `static/`, or `build/` of its own anymore — they all live in `packages/desktop/`.

## Development Workflow

All commands run from the repo root. The root `package.json` proxies every
desktop-specific script to `packages/desktop` via `pnpm --filter marktext`,
so the names and behavior are unchanged from the pre-monorepo layout.

```bash
# Install dependencies (runs scripts/postinstall.ts automatically — patches
# native-keymap, downloads Electron, rebuilds native modules, minifies locales)
pnpm install

# Run in development mode
# Renderer hot-reloads automatically. Pressing Ctrl+R in the dev window reloads
# the renderer (which re-runs the preload script); changes to the main process
# require restarting `pnpm run dev`.
pnpm run dev

# Build and preview the application (electron-vite preview rebuilds by default).
pnpm run start

# Build without packaging — fast path for verifying the renderer/main compile
pnpm run build:unpack

# Auto-format the repo with Prettier (separate from `lint`, which only checks)
pnpm run format

# Minify locale files (required for production builds, skip during dev)
pnpm run minify-locales

# Performance debugging — builds, then launches with a Node inspector on :5858
pnpm run perf:inspect       # attach when ready
pnpm run perf:inspect-brk   # break on first line

# Website (validated and deployed by website-deploy.yml)
pnpm --filter marktext-website dev      # Next.js dev server
pnpm --filter marktext-website build    # Next.js build → packages/website/.next/
```

To launch existing bundles without rebuilding, run
`pnpm -C packages/desktop exec electron-vite preview --skipBuild`.

If you need to invoke a script directly inside a package, use
`pnpm --filter <name> <script>` or `pnpm -C packages/<name> <script>`.

## Build Commands

```bash
pnpm run build:win    # Windows x64 — NSIS installer + zip
pnpm run build:mac    # macOS host architecture — DMG + zip
pnpm run build:linux  # Linux — AppImage, snap, deb, rpm, tar.gz
```

Use `build:win:x64`, `build:win:arm64`, `build:mac:x64`, `build:mac:arm64`,
`build:linux:x64` or `build:linux:arm64` to select an architecture explicitly.
Linux arm64 omits snap.

All platform build scripts automatically run `minify-locales` and `electron-rebuild` before packaging.

## Testing

```bash
pnpm run test          # Desktop unit tests (Vitest)
pnpm run test:unit     # Desktop unit tests only
pnpm run test:e2e      # End-to-end tests (Playwright)
pnpm run lint          # ESLint (run before committing; CI enforces)
pnpm run typecheck     # vue-tsc --noEmit (CI enforces)

# Run a single spec — paths are relative to packages/desktop. Use `-C` so
# pnpm resolves the spec path inside the desktop package's vitest config.
pnpm -C packages/desktop exec vitest run test/unit/specs/markdown-basic.spec.ts
pnpm -C packages/desktop exec vitest run -t 'partial test name'

# Single Playwright spec (playwright.config.ts sits at packages/desktop/, so it
# is picked up automatically — run these from that package, not the repo root)
pnpm -C packages/desktop exec playwright test test/e2e/launch.spec.ts
pnpm -C packages/desktop exec playwright test -g 'partial test name'
```

Root test commands cover desktop only. Muya checks use
`pnpm --filter @muyajs/core test` and `pnpm --filter @muyajs/core test:spec`;
its browser suite uses `pnpm --filter muya-e2e e2e:chromium`. Website checks use
`pnpm --filter marktext-website lint` and `pnpm --filter marktext-website type-check`.

## Code Style

Enforced by ESLint + Prettier. Run `pnpm run lint` and `pnpm run typecheck` before committing.

- 2-space indentation
- No semicolons
- Single quotes
- TypeScript with `strict: true`; see `packages/website/content/docs/dev/TYPESCRIPT.md`
- Cross-process types live in `packages/desktop/src/shared/types/`; ambient declarations in `packages/desktop/src/types/`
- IPC channels are typed via the contract in `packages/desktop/src/shared/types/ipc.ts`
- The renderer is fully sandboxed — every IPC and Node access goes through `window.electron.*` / `window.fileUtils.*` etc. (typed in `packages/desktop/src/types/global.d.ts`)

### Conditions and operation handlers

- Keep conditions and logical expressions free of state-changing operations. Determine whether an operation applies with a pure query or prepared data, then execute it explicitly in the corresponding branch.
- Do not combine mutation and dispatch in a helper that returns a boolean meaning "handled" or "not handled". Operation handlers should return `void` / `Promise<void>`; keep their applicability checks separate.
- Avoid patterns such as `if (writeSelectedImage(...))`, `if (await tryPasteImage(...))`, or `condition && performOperation()`. Put the operation in the branch body instead.
- External APIs may report success or failure. Capture that result in a variable before checking it; do not hide the API call inside the condition.

### Comments

Follow `.github/COMMENTING-GUIDELINES.md` for every comment you write. The core rule: a comment must describe what isn't obvious from the code — rationale, units, invariants, ownership, the abstraction a caller needs — never restate the code or echo the words already in the name. Before finishing any change, review the comments you added or touched against that document, and delete any that only repeat the code. Prefer self-explanatory names over comments; when a comment is genuinely needed, keep it short and complete and place it next to the code it describes.

## Architecture: Main, Preload and Renderer

Electron application code lives in `packages/desktop/`. Preload executes in a
renderer context rather than its own OS process. Muya is a separate
workspace package that the renderer (and tests) consume as `@muyajs/core`.

```
main process  (packages/desktop/src/main/)
  ├── Full Node.js + Electron API access
  ├── IO, file system, native dialogs, auto-updater, spell checker
  ├── One main process per application launch
  └── Controls editor windows via IPC

preload  (packages/desktop/src/preload/)
  ├── Bridge between main and renderer
  ├── Editor and preferences windows use contextIsolation: true,
  │   sandbox: true and nodeIntegration: false (src/main/config.ts)
  └── Compiled to CommonJS

renderer  (packages/desktop/src/renderer/)
  ├── One process per editor window (spawned by main)
  ├── Vue 3 + Pinia — all UI state and editor interaction
  ├── Hosts both Muya (WYSIWYG) and CodeMirror (source-code mode)
  └── Compiled to ES Modules only

Muya  (packages/muya/)              ← workspace package @muyajs/core
  ├── Self-contained editor backend, TypeScript
  ├── No Electron APIs
  └── Handles markdown parsing, block data structure, document export, rendering
```

## IPC Conventions

Most IPC channels between main and renderer use the `mt::` prefix (e.g. `mt::open-new-tab`, `mt::file-saved`). Some internal channels do not follow this convention (e.g. `language-changed`).

See `packages/website/content/docs/dev/IPC.md` for conventions and examples.

## Further Reading

`packages/website/content/docs/dev/` contains the deeper developer documentation referenced by this guide. Same files are published as the developer docs section on <https://marktext.me/docs/dev/overview>:

- `ARCHITECTURE.md` — process/module layering beyond the summary above
- `BUILD.md` — full platform build prerequisites (including the Arch Linux deps added recently)
- `DEBUGGING.md` — attaching debuggers to main/renderer processes
- `INTERFACE.md` — Muya and renderer public interfaces
- `IPC.md` — full IPC channel catalog and `mt::` conventions
- `LINUX_DEV.md` — Linux-specific dev environment setup
- `PERFORMANCE.md` — perf measurement workflow (pairs with `pnpm run perf:inspect`)
- `RELEASE.md` / `RELEASE_HOTFIX.md` — release process

## Important Build Notes

- **CommonJS vs ESM**: `main` and `preload` compile to CommonJS; `renderer` is ESM-only. Do not use `require()` in renderer code.
- **Minify locales**: `pnpm run minify-locales` must run before production builds. It is included in `build:unpack` and `build:win/mac/linux`, but not in `build` or `dev`.
- **Native modules**: After changing Electron version, run `pnpm run rebuild-native` (`electron-rebuild -f`).
- **Hot reload**: The renderer hot-reloads via Vite HMR. `Ctrl+R` in the dev window reloads the renderer and re-runs the preload script. Changes to `main/` source are NOT picked up by a window reload — restart `pnpm run dev` to pick them up.
- **electron-builder output**: `directories.output` in `packages/desktop/electron-builder.yml` is set to `../../dist` so installers land in the repo-root `dist/` (where CI artifact globs look for them). `out/` from electron-vite stays inside `packages/desktop/`.
- **Path aliases** (defined in `packages/desktop/electron.vite.config.ts`, mirrored in `vitest.config.ts` and `tsconfig.base.json`):
  - `@` → `packages/desktop/src/renderer/src`
  - `common` → `packages/desktop/src/common`
  - `@shared` → `packages/desktop/src/shared`
- **`@muyajs/core` resolution**: not a path alias — electron-vite and Vitest resolve it through the package's `exports` map, which points at `packages/muya/src/index.ts`. TypeScript would then pull the whole muya tree into the desktop's program, so `tsconfig.base.json` redirects the types to `../muya/lib/types/index.d.ts`; `pnpm typecheck` and `postinstall` both run `pnpm --filter @muyajs/core build:types` to emit that (git-ignored) directory.
- **Patches**: `patch-package` patches live at `packages/desktop/patches/`. The root `postinstall` calls patch-package with `cwd=packages/desktop` so the path resolves correctly.

## Contribution

- Submit PRs to the **`develop`** branch (not `main`).
- Do not commit screenshots or recordings created as PR evidence to the repository. Keep these artifacts outside the checkout and upload them directly to the PR when needed; do not add them under `docs/assets/` or other tracked directories.
- Reference the related issue in the PR description.
- Run `pnpm run lint` before submitting.
- All PRs must pass CI before merge.
- See `.github/CONTRIBUTING.md` for the full contributing guide.

## Cursor Cloud Specific Instructions

This repository ships a custom Cloud Agent environment in `.cursor/environment.json` (Ubuntu 24.04 with Node 22, pnpm 10, `node-gyp`, Xvfb + Electron libs, Playwright Chromium, Rust stable + `wasm32-unknown-unknown`, `wasm-bindgen-cli 0.2.130`, and Linux GPUI/Vulkan/X11/Wayland headers).

### Rust / GPUI Incremental Migration Context (Read Before Starting Migration Work)

This fork is executing a **Strangler-Fig Electron → Rust (GPUI)** migration while preserving full legacy MarkText functionality. Before implementing or reviewing any migration slice, read:

- `docs/migration/ARCHITECTURE.md` — measured subsystem map (`~104k LOC`), feature inventory, IPC/Pinia boundaries, and the Outline-click → Editor-scroll reference trace.
- `docs/migration/MIGRATION_PLAN.md` — the Two-Stage **`gpui_web` (WASM in persistent `<iframe>`) → Native OS GPUI** strategy, `HostBridge` trait contract, `gpui_web` constraints, dependency DAG, and governance rules.
- `docs/migration/STATUS.md` — live slice tracker (Slice ID, feature flag key, prerequisites, target crate, and status). Check your slice's prerequisites and update its status row in your PR.
- `docs/migration/RUNBOOK.md` — **mandatory step-by-step execution protocol** for running a migration slice end-to-end (Before baseline → early Draft PR → implementation → After benchmark & screenshots → tag `@shashu10`).

**Mandatory Cloud Agent Slice Sequence (See `docs/migration/RUNBOOK.md` for Full Template):**

1. **Check prereqs & claim:** Verify slice prerequisites in `STATUS.md` and check open `migration` PRs (`gh pr list --label migration`) to avoid parallel collisions; branch off `develop` and mark `in-progress`.
2. **Capture "Before" baseline FIRST:** Build untouched legacy app and record startup/pane latency, Main+Renderer RSS memory, 5s idle CPU %, bundle size, and a Before screenshot in `/tmp/` (never commit PNGs to git).
3. **Open a Draft PR early:** Push the `STATUS.md` claim and open a Draft PR (`--draft --base develop --label migration`) containing the **Before baseline table** and **Tentative Implementation Plan** so measurements survive context compaction and lock the slice.
4. **Implement behind feature flag:** Build the Rust GPUI crate against `HostBridge` + `gpui-ce` (`default-features = false`), keep `prod` on `legacy`, and add Rust + IPC parity tests.
5. **Verify & capture "After" metrics:** Run `pnpm lint`, `pnpm typecheck`, `pnpm test:unit`, `cargo test`, `cargo clippy`, `wasm32` build, repeat the exact same measurements + After screenshot, and verify `flag=legacy` rollback.
6. **Finalize PR & request human review:** Fill in **Planned vs. Actually Implemented**, **Before vs. After scorecard** (honestly distinguishing Stage 1 WASM overhead guardrails vs. Stage 2 native ROI), and screenshot artifact links; run `gh pr ready`, tag **`@shashu10`**, and **never self-merge**.

**Key Migration Invariants for All Agents:**

- **Write GPUI views once:** Target `gpui-ce` (`default-features = false`) + `gpui_ce_platform` (`default-features = false`) so every view compiles cleanly to both `wasm32-unknown-unknown` (Stage 1) and native OS GPUI (Stage 2) with no platform `#[cfg]` inside UI views.
- **Route all host I/O through `HostBridge`:** Never bypass the shared preference/IPC bridge or mutate disconnected local state; preference writes must sync to Electron Main and broadcast to open Editor windows.
- **Gate every slice behind a feature flag** (`legacy` | `rust-wasm` | `gpui-native` | `shadow`) and preserve parity in both WYSIWYG and Source Code modes where applicable.

### Cloud VM Verification & PR Rules

- **Headless Electron E2E & visual QA:** Always wrap Electron/Playwright commands in `xvfb-run --auto-servernum`, e.g.:
  - `xvfb-run --auto-servernum pnpm test:e2e`
  - `xvfb-run --auto-servernum pnpm -C packages/desktop exec playwright test test/e2e/launch.spec.ts`
- **Native rebuilds:** Keep `USE_ELECTRON_CLANG=0` set if running `pnpm run rebuild-native` or `pnpm tsx scripts/postinstall.ts`.
- **Rust / GPUI WASM builds:** Compile Stage 1 web panes with `cargo build --target wasm32-unknown-unknown --release` and emit JS/WASM bindings via `wasm-bindgen --target web`.
- **PR & Governance rules for migration slices:**
  - Always open PRs targeting **`develop`** on this fork (never push directly to `develop` or auto-merge).
  - Tag migration PRs with the `migration` label.
  - Run `pnpm run lint`, `pnpm run typecheck`, `pnpm test:unit`, and relevant Rust/Playwright checks before finishing.
  - Keep screenshot artifacts outside the tracked git tree and attach/upload them as PR evidence.
