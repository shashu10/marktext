import type { ChangeAuditContext } from './change-detector.ts'
import type { ExistingRegressionIssue } from './issue-filer.ts'
import { formatRubricMarkdown } from './rubric.ts'

export interface QaPromptInput {
  repoSlug: string
  context: ChangeAuditContext
  openIssues: ExistingRegressionIssue[]
}

export function buildQaAuditPrompt(input: QaPromptInput): string {
  const { repoSlug, context, openIssues } = input

  const commitLines = context.commits.length > 0
    ? context.commits
      .map((c) => {
        const prSuffix = c.prNumber ? ` (PR #${c.prNumber})` : ''
        return `- \`${c.shortSha}\` ${c.subject}${prSuffix} — ${c.author}`
      })
      .join('\n')
    : '- Manual / baseline audit against current `HEAD`.'

  const changedFileLines = context.changedFiles.length > 0
    ? context.changedFiles.slice(0, 60).map((f) => `- \`${f}\``).join('\n')
    : '- (Full active slice audit)'

  const sliceTable = context.targetSlices
    .map(
      (s) =>
        `- **${s.id}** (${s.name}) | Flag: \`${s.flag}\` | Rust crate: \`${s.rustCrate}\` | Legacy: ${s.legacyPaths.map((p) => `\`${p}\``).join(', ')}`
    )
    .join('\n')

  const openIssueLines = openIssues.length > 0
    ? openIssues
      .map((issue) => `- #${issue.number} [${issue.fingerprint ?? 'no-fingerprint'}]: ${issue.title}`)
      .join('\n')
    : '- None currently open.'

  const measureCommands = context.targetSlices
    .map((s) => [
      `# Slice ${s.id} — Legacy Baseline (Before)`,
      `xvfb-run --auto-servernum --server-args="-screen 0 1280x800x24" pnpm migration:measure --slice ${s.id} --phase before --flag legacy --out /tmp/qa-${s.id}-before.json --screenshot /tmp/qa-${s.id}-before.png`,
      `# Slice ${s.id} — Migrated Rust/GPUI View (After)`,
      `xvfb-run --auto-servernum --server-args="-screen 0 1280x800x24" pnpm migration:measure --slice ${s.id} --phase after --flag rust-wasm --out /tmp/qa-${s.id}-after.json --compare /tmp/qa-${s.id}-before.json --screenshot /tmp/qa-${s.id}-after.png`
    ].join('\n'))
    .join('\n\n')

  const diffRange = context.baseSha
    ? `${context.baseSha.slice(0, 10)}..${context.headSha.slice(0, 10)}`
    : `${context.headSha.slice(0, 10)} (baseline)`

  return `You are an autonomous **QA Engineer / Technical Program Manager (TPM)** auditing the MarkText Electron → Rust/GPUI migration on \`${repoSlug}\`.

## STRICT NON-NEGOTIABLE GUARDRAILS
1. **DO NOT FIX ANY BUGS.** You are strictly QA/TPM.
2. **DO NOT edit, create, or delete any tracked source files in the repository.**
3. **DO NOT run \`git commit\`, \`git push\`, or open any pull request.**
4. Write temporary verification outputs (JSON/PNG/logs) ONLY under \`/tmp/\`.
5. Only report genuine, verified regressions supported by concrete evidence from running the app, inspecting screenshots, checking DOM/canvas/logs, or inspecting the PR diff. Do not hallucinate issues.

---

## Audit Scope
- **Commit range:** \`${diffRange}\`
- **Target slices to audit:**
${sliceTable}

### Recent Commits / PRs Since Last Audit
${commitLines}

### Changed Files
${changedFileLines}

### Currently Open Regression Issues (Do Not File Duplicates — Still Report With Matching Fingerprint Only If Helpful)
${openIssueLines}

---

## Step-by-Step QA Verification Protocol

Execute these checks in order inside the VM:

1. **Ensure artifacts are built (if missing in workspace):**
   - Check if the WASM host artifacts under \`packages/desktop/src/renderer/public/gpui-host/\` and Electron build under \`packages/desktop/out/\` exist; if missing or stale for the audited commit, build the Rust WASM host and desktop app (\`pnpm --filter marktext build\` or the crate's wasm-pack/wasm-bindgen build script — inspect \`package.json\` and \`rust/\` first).
2. **Capture Before (Legacy) vs. After (Migrated) Evidence:**
   Run the deterministic measurement & screenshot harness for each target slice:
   \`\`\`bash
   ${measureCommands}
   \`\`\`
   Inspect the JSON reports (\`/tmp/qa-<slice>-before.json\`, \`/tmp/qa-<slice>-after.json\`) for startup latency, pane-switch latency, RSS memory, idle CPU %, and WASM bundle size.
3. **Visual & Functional Parity Inspection:**
   - **Visually inspect BOTH screenshots** (\`/tmp/qa-<slice>-before.png\` and \`/tmp/qa-<slice>-after.png\`) using your image-reading tool. Check specifically for:
     - **Blank or black screen** / clipped canvas / unrendered fonts (\`P0\`)
     - **Scrolling behavior:** Can the user scroll all the way to the bottom of the migrated pane with mouse wheel / scrollbars without resizing the window? Test scrolling programmatically via Playwright or inspecting scroll containers if needed (\`P1\` if broken with resize workaround).
     - **Control parity:** Did every control from the legacy view make it across? Did a dropdown turn into a list of buttons (\`P2\`)? Are labels, toggles, text inputs, and descriptions present?
     - **Visual polish:** Button border-radius, font spacing/kerning, padding, alignment, colors (\`P3\`).
   - **Behavioral & Rollback Verification:**
     - Check whether toggling/editing controls persists to preferences and whether \`flag=legacy\` cleanly renders the original Vue view (\`P0\` if broken).
     - Check console errors, uncaught exceptions, WebGL/WASM panics, or missing IPC handlers.
4. **Code / PR Diff Review:**
   - Review the diff for \`${diffRange}\` (and legacy vs. Rust view implementation for each audited slice) to confirm root cause and exact file/line references for any observed regression.

---

## Severity Classification Rubric (Apply Strictly)

${formatRubricMarkdown()}

---

## Required Final Output Format

At the very end of your response, emit **one** fenced \`qa-findings-json\` block matching this exact schema (use \`"findings": []\` if zero regressions were found):

\`\`\`qa-findings-json
{
  "summary": "One-paragraph QA executive summary covering what was tested, metrics delta, and overall verdict.",
  "slicesAudited": ["A1"],
  "findings": [
    {
      "title": "Short, crisp bug title (no [P0] prefix — added automatically)",
      "severity": "P0 | P1 | P2 | P3",
      "slice": "A1",
      "category": "blank-or-crash | data-or-rollback | interaction-or-scroll | performance | component-divergence | copy-or-layout | visual-polish",
      "fingerprint": "stable-kebab-slug-unique-to-this-bug",
      "summary": "Clear 1-2 sentence description of the regression.",
      "expected": "Exact behavior/appearance in the legacy Electron/Vue view.",
      "actual": "Exact behavior/appearance observed in the migrated Rust/GPUI view.",
      "reproSteps": [
        "Launch MarkText with MARKTEXT_MIGRATION_OVERRIDES=prefs.pane.general=rust-wasm",
        "Open Preferences -> General",
        "Attempt to scroll down..."
      ],
      "workaround": "Resize the preferences window vertically (or null if none)",
      "rubricJustification": "Why this maps to P0/P1/P2/P3 under the QA rubric.",
      "evidence": [
        "Before/After screenshot comparison: ...",
        "Metrics or console log excerpt: ...",
        "Relevant file/symbol: rust/crates/..."
      ],
      "relatedPrs": [5]
    }
  ]
}
\`\`\`

Remember: **Triage and report only. Leave \`git status\` completely clean.**
`
}
