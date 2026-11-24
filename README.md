<p align="center"><img src="docs/assets/logo-small.png" alt="MarkText" width="100" height="100"></p>

<h1 align="center">MarkText</h1>

## Incremental Electron → Rust (GPUI) Migration Prototype

This fork demonstrates an enterprise **Strangler-Fig migration workflow** built with **Cursor Cloud Agents (custom environment)** and **event-driven `@cursor/sdk` GitHub Actions workflows** on a live `~125k LOC` Electron + Vue 3 desktop application. All 8 Preferences slices (`A1`–`A8`, `99.56%` of the Preferences surface) compile from a single set of Rust/GPUI crates into both **Stage 1 WASM (`gpui_web` inside Electron)** and **Stage 2 Native OS GPUI (`mt-pref-app`)**, gated behind multi-environment feature flags (`prod` defaults to `legacy` for instant rollback).

**Read the 2-page technical write-up:** [`docs/WRITEUP.md`](docs/WRITEUP.md) · **Migration docs:** [`MIGRATION_PLAN.md`](docs/migration/MIGRATION_PLAN.md) · [`STATUS.md`](docs/migration/STATUS.md) · [`RUNBOOK.md`](docs/migration/RUNBOOK.md) · [`MEMORY.md`](docs/migration/MEMORY.md) · [`QA_RUBRIC.md`](docs/migration/QA_RUBRIC.md)

### Run the Prototype in One Command

**One-time local prerequisites (macOS or Linux desktop):** Node `22.18+` (or `24`), `pnpm 10.33.4`, Rust `1.95+` with Cargo, and platform native build tools (Xcode CLI tools + Python 3 on macOS). Then install dependencies once:

```sh
pnpm install --frozen-lockfile
```

*(Only needed if you also want to test the Stage 1 `rust-wasm` mode: `rustup target add wasm32-unknown-unknown && cargo install --locked --version 0.2.130 wasm-bindgen-cli`.)*

**Launch in one command from the repo root (no Cursor API key required):**

```sh
pnpm migration:demo              # Stage 2 Native GPUI Preferences window (W2.A)
pnpm migration:demo rust-wasm    # Stage 1 GPUI-in-WASM iframe across A1–A8
pnpm migration:demo legacy       # Instant rollback to original Electron/Vue Preferences
```

`pnpm migration:demo` builds the Rust target and Electron bundles, launches MarkText with a disposable `/tmp/marktext-demo.*` profile (discarded on quit so your personal settings are untouched), and keeps `MARKTEXT_MIGRATION_ENV=prod` with explicit runtime flag overrides. Open **Preferences** (`Cmd+,` on macOS / `Ctrl+,` on Linux), navigate across **General → Appearance → Markdown → Keybindings**, and toggle a preference to verify live persistence into Electron Main.

### Cursor Cloud Agent Environment Setup

1. **Connect this fork (`develop` branch) in Cursor Cloud Agents.** Cursor automatically loads [`.cursor/environment.json`](.cursor/environment.json), building [`.cursor/Dockerfile`](.cursor/Dockerfile) (Ubuntu 24.04, Node 22, pnpm `10.33.4`, Rust stable + `wasm32-unknown-unknown` + `wasm-bindgen-cli 0.2.130`, `lld` fast linker with `[build] jobs = 3`, Xvfb, Mesa Lavapipe Vulkan ICD, `scrot`, `fonts-liberation`) and pre-warming dependencies via [`.cursor/install.sh`](.cursor/install.sh) (`pnpm build:unpack`, `migration:build-wasm`, `migration:build-native-pref`, `cargo test --no-run`).
2. **Recommended right-sized container spec** (from our live `W2.A` Cloud VM profiling audit in [PR #23](https://github.com/shashu10/marktext/pull/23)): **4–5 vCPU**, **8–12 GiB RAM** (with `lld` + `[profile.dev.package."*"] debug = false` + `CARGO_BUILD_JOBS=3`), **50–64 GiB disk**, and **≥1 GiB `/dev/shm`**.
3. **Repo-native Skills & Scoped Rules:** Cloud Agents automatically pick up [`.cursor/rules/rust-gpui-panes.mdc`](.cursor/rules/rust-gpui-panes.mdc) (`rust/**/*.rs`), [`.cursor/rules/parallel-slice-hygiene.mdc`](.cursor/rules/parallel-slice-hygiene.mdc), and Project Skills [`.cursor/skills/migrate-slice/SKILL.md`](.cursor/skills/migrate-slice/SKILL.md) and [`.cursor/skills/verify-migration-slice/SKILL.md`](.cursor/skills/verify-migration-slice/SKILL.md).
4. **Headless verification inside the Cloud VM** (no API key needed):
   ```sh
   xvfb-run --auto-servernum pnpm migration:measure --slice W2.A --flag gpui-native --out /tmp/w2a.json --screenshot /tmp/w2a.png
   ```
5. **Optional CI secret for `@cursor/sdk` GitHub Actions workflows:** Add `CURSOR_API_KEY` as a GitHub Actions repository secret to enable the three decoupled SDK workflows ([`migration-qa-monitor.yml`](.github/workflows/migration-qa-monitor.yml), [`issue-triage.yml`](.github/workflows/issue-triage.yml), [`legacy-sync.yml`](.github/workflows/legacy-sync.yml)). Each runs a `~3s`, `$0` deterministic change/drift gate before invoking `@cursor/sdk`.

### Built & Working vs. Design Only

| Capability | Status | Evidence / Notes |
| --- | --- | --- |
| **Multi-env feature flag router** (`dev`/`staging`/`prod`, `prod=legacy`, instant rollback) | **Built & Working** | `packages/desktop/src/shared/flags/` ([PR #3](https://github.com/shashu10/marktext/pull/3)) |
| **Stage 1 GPUI-in-WASM (`A1`–`A8`)** inside persistent `<GpuiPaneHost>` iframe | **Built & Working** | All 8 Preferences slices merged ([PR #5](https://github.com/shashu10/marktext/pull/5), [#9](https://github.com/shashu10/marktext/pull/9)–[#14](https://github.com/shashu10/marktext/pull/14), [#17](https://github.com/shashu10/marktext/pull/17), [#20](https://github.com/shashu10/marktext/pull/20), [#34](https://github.com/shashu10/marktext/pull/34)); `99.56%` of Preferences (`11,605` Rust LOC) |
| **Stage 2 Native GPUI Preferences Window (`W2.A`)** over NDJSON stdio (`mt-pref-app`) | **Built & Working** | Merged ([PR #21](https://github.com/shashu10/marktext/pull/21)); macOS Metal startup deadlock fixed ([PR #22](https://github.com/shashu10/marktext/pull/22)); runs on macOS Metal & Linux Lavapipe |
| **Custom Cloud Agent environment, live profiling, Skills & Scoped Rules** | **Built & Working** | `.cursor/{Dockerfile, environment.json, install.sh, rules/, skills/}` ([PR #1](https://github.com/shashu10/marktext/pull/1), [PR #23](https://github.com/shashu10/marktext/pull/23)) |
| **Deterministic measurement (`migration:measure`) & `% migrated` (`migration:progress`)** | **Built & Working** | `scripts/migration-measure.ts`, `scripts/migration-progress.ts`, `docs/migration/slices.json` ([PR #2](https://github.com/shashu10/marktext/pull/2), [#4](https://github.com/shashu10/marktext/pull/4), [#19](https://github.com/shashu10/marktext/pull/19)) |
| **Hourly change-gated QA/TPM Monitor (`@cursor/sdk`)** | **Built & Live-Proven** | `.github/workflows/migration-qa-monitor.yml` ([PR #7](https://github.com/shashu10/marktext/pull/7)); live run auto-filed **9 real issues** (`#25`–`#33`); artifact-backed retry recovery in open [PR #24](https://github.com/shashu10/marktext/pull/24) |
| **Decoupled Issue Triage (`issue-triage.yml`) & Legacy-to-Rust Parity Sync (`legacy-sync.yml`)** | **Built & Unit-Tested** | Shipped in [`13dae1df`](https://github.com/shashu10/marktext/commit/13dae1df) with `docs/migration/OWNERS.json` & Vitest/Node unit tests; live end-to-end GitHub Actions execution not yet soaked on production traffic |
| **Dark-theme parity & remaining `P1`/`P2` polish for Preferences `prod` cutover** | **Open Backlog** | Tracked transparently in [Issue #18](https://github.com/shashu10/marktext/issues/18) and `#25`–`#32` (`prod` intentionally remains on `legacy`) |
| **Waves 1B–4** (Dialogs, Editor Chrome, Main Process Services, `@muyajs/core` WYSIWYG, Electron retirement) | **Design Only** | Architectural DAG & recalibrated estimates in `docs/migration/{MIGRATION_PLAN, STATUS}.md` and `docs/WRITEUP.md` |
| **Runtime `shadow` diffing, enforced branch protection / `CODEOWNERS`, & enterprise MCP/RAG context (Linear/Confluence/Datadog)** | **Design Only** | Flag schema reserves `shadow`; branch protection, multi-OS CI runners, and RBAC MCP/RAG connectors documented as enterprise rollout steps |

---

<div align="center">
  <a href="https://twitter.com/intent/tweet?via=marktextme&url=https://github.com/marktext/marktext/&text=What%20do%20you%20want%20to%20say%20to%20app?&hashtags=happyMarkText">
    <img src="https://img.shields.io/twitter/url/https/github.com/marktext/marktext.svg?style=for-the-badge" alt="twitter">
  </a>
</div>
<div align="center">
  <strong>:high_brightness: Next generation markdown editor :crescent_moon:</strong><br>
  A simple and elegant open-source markdown editor that focused on speed and usability.<br>
  <sub>Available for Linux, macOS and Windows.</sub>
</div>

<br>

<div align="center">
  <!-- License -->
  <a href="LICENSE">
    <img src="https://img.shields.io/github/license/marktext/marktext.svg" alt="LICENSE">
  </a>
  <!-- Downloads total -->
  <a href="https://github.com/marktext/marktext/releases">
    <img src="https://img.shields.io/github/downloads/marktext/marktext/total.svg" alt="total download">
  </a>
  <!-- Downloads latest release -->
  <a href="https://github.com/marktext/marktext/releases/latest">
    <img src="https://img.shields.io/github/downloads/marktext/marktext/latest/total.svg" alt="latest download">
  </a>
  <!-- sponsors -->
  <a href="https://opencollective.com/marktext">
    <img src="https://opencollective.com/marktext/tiers/silver-sponsors/badge.svg?label=SilverSponsors&color=brightgreen" alt="sponsors">
  </a>
</div>

<div align="center">
  <h3>
    <a href="https://marktext.me">
      Website
    </a>
    <span> | </span>
    <a href="https://github.com/marktext/marktext#features">
      Features
    </a>
    <span> | </span>
    <a href="https://github.com/marktext/marktext#download-and-installation">
      Downloads
    </a>
    <span> | </span>
    <a href="https://github.com/marktext/marktext#development">
      Development
    </a>
    <span> | </span>
    <a href="https://github.com/marktext/marktext#contribution">
      Contribution
    </a>
  </h3>
</div>

<div align="center">
  <sub>Translations:</sub>
  <a href="docs/i18n/README-zh_cn.md#readme">
    <span>:cn:</span>
  </a>
  <a href="docs/i18n/README-zh_tw.md#readme">
    <span>:taiwan:</span>
  </a>
  <a href="docs/i18n/README-jp.md#readme">
    <span>:jp:</span>
  </a>
  <a href="docs/i18n/README-fr.md#readme">
    <span>:fr:</span>
  </a>
  <a href="docs/i18n/README-tr.md#readme">
    <span>:tr:</span>
  </a>
  <a href="docs/i18n/README-es.md#readme">
    <span>:es:</span>
  </a>
  <a href="docs/i18n/README-pt.md#readme">
    <span>:portugal:</span>
  </a>
  <a href="docs/i18n/README-kr.md#readme">
    <span>:kr:</span>
  </a>
  <a href="docs/i18n/README-bn.md#readme">
    <span>:bangladesh:</span>
  </a>
</div>

<div align="center">
  <sub>This Markdown editor that could. Built with ❤︎ by
    <a href="https://github.com/Jocs">Jocs</a> and
    <a href="https://github.com/marktext/marktext/graphs/contributors">
      contributors
    </a>
    .
  </sub>
</div>

<br />

<h2 align="center">Sponsors</h2>

MarkText is an open-source Markdown editor powered by the support of its community. If MarkText improves your workflow, please consider [sponsoring the project](https://github.com/sponsors/marktext). Thank you to all the sponsors ❤️

**Special Sponsor**

<table>
<tr>
<td width="182"><a href="https://serpapi.com/?utm_source=marktext"><img src="docs/assets/sponsors/serpapi.png" width="150"></a></td>
<td align="left"><a href="https://serpapi.com/?utm_source=marktext">Scrape Google and other search engines from our fast, easy, and complete API.</a></td>
</tr>
</table>

<!-- current-sponsors:start -->

**Current Public Sponsors (1)**

[![Current public sponsors](docs/assets/sponsors/current.svg)](https://github.com/sponsors/marktext)

Only active, public GitHub sponsors are listed here. Thank you to our private sponsors as well.

<!-- current-sponsors:end -->

## Screenshot

![](docs/assets/marktext.png?raw=true)

## Features

- Realtime preview (WYSIWYG) and a clean and simple interface to get a distraction-free writing experience.
- Support [CommonMark Spec](https://spec.commonmark.org), [GitHub Flavored Markdown Spec](https://github.github.com/gfm/) and selective support [Pandoc markdown](https://pandoc.org/MANUAL.html#pandocs-markdown).
- Markdown extensions such as math expressions (KaTeX), front matter and emojis.
- Support paragraphs and inline style shortcuts to improve your writing efficiency.
- Output **HTML** and **PDF** files.
- Various [themes](https://marktext.me/docs/themes): **Cadmium Light**, **Material Dark** etc.
- Various editing modes: **Source Code mode**, **Typewriter mode**, **Focus mode**.
- Paste images directly from clipboard.

## Download and Installation

![platform](https://img.shields.io/static/v1.svg?label=Platform&message=Linux%20x64%2Farm64%20|%20macOS%20x64%2Farm64%20|%20Windows%20x64%2Farm64&style=for-the-badge)

|               ![](https://raw.githubusercontent.com/wiki/ryanoasis/nerd-fonts/screenshots/v1.0.x/mac-pass-sm.png)               |               ![](https://raw.githubusercontent.com/wiki/ryanoasis/nerd-fonts/screenshots/v1.0.x/windows-pass-sm.png)               |              ![](https://raw.githubusercontent.com/wiki/ryanoasis/nerd-fonts/screenshots/v1.0.x/linux-pass-sm.png)              |
| :-----------------------------------------------------------------------------------------------------------------------------: | :---------------------------------------------------------------------------------------------------------------------------------: | :-----------------------------------------------------------------------------------------------------------------------------: |
| [![Download for macOS](https://img.shields.io/badge/macOS-Download-blue)](https://github.com/marktext/marktext/releases/latest) | [![Download for Windows](https://img.shields.io/badge/Windows-Download-blue)](https://github.com/marktext/marktext/releases/latest) | [![Download for Linux](https://img.shields.io/badge/Linux-Download-blue)](https://github.com/marktext/marktext/releases/latest) |

Want to see new features of the latest version? Please refer to [CHANGELOG](https://marktext.me/docs/changelog).

#### macOS

Requires macOS 13 (Ventura) or later. Universal builds aren't published — pick the matching `arm64` or `x64` installer.

You can either download the latest `marktext-mac-(arm64|x64)-%version%.dmg` from the [release page](https://github.com/marktext/marktext/releases/latest) or install MarkText using [**homebrew cask**](https://github.com/caskroom/homebrew-cask). To use Homebrew-Cask you just need to have [Homebrew](https://brew.sh/) installed.

```bash
brew install --cask mark-text
```

#### Windows

Requires Windows 10 or 11. Both x64 and arm64 installers are published — pick the architecture that matches your machine.

Simply download and install MarkText via the setup wizard (`marktext-win-(x64|arm64)-%version%-setup.exe`) and choose whether to install per-user or machine wide. Alternatively, install MarkText using a package manager such as [Chocolatey](https://chocolatey.org/) or [Winget](https://docs.microsoft.com/en-us/windows/package-manager/winget/).

To use Chocolatey, you need to have [Chocolatey](https://chocolatey.org/install) installed:

```bash
choco install marktext
```

To use Winget, you need to have [Winget](https://docs.microsoft.com/en-us/windows/package-manager/winget/#install-winget) installed:

```bash
winget install marktext
```

#### Linux

Please follow the [Linux installation instructions](https://marktext.me/docs/installation).

#### Other

All binaries for Linux, macOS and Windows can be downloaded from the [release page](https://github.com/marktext/marktext/releases/latest). If a version is unavailable for your system, then please open an [issue](https://github.com/marktext/marktext/issues).

## Development

If you wish to build MarkText yourself, please check out our [build instructions](https://marktext.me/docs/dev/build).

- [User documentation](https://marktext.me/docs/introduction)
- [Developer documentation](https://marktext.me/docs/dev/overview)

If you have any questions regarding MarkText, you are welcome to write an issue. When doing so please use the default format found when opening an issue. Of course, if you submit a PR directly, it will be greatly appreciated.

## Contribution

MarkText is in development, please make sure to read the [Contributing Guide](.github/CONTRIBUTING.md) before making a pull request. Want to add some features to MarkText? Refer to our [roadmap](https://github.com/marktext/marktext/projects) and open issues.

## Contributors

Thank you to all the people who have already contributed to MarkText[[contributors](https://github.com/marktext/marktext/graphs/contributors)].

<a href="https://github.com/marktext/marktext/graphs/contributors"><img src="https://opencollective.com/marktext/contributors.svg?width=890" /></a>

## License

[**MIT**](LICENSE).
