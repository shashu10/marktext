import type { CommitSummary } from '../qa-monitor/change-detector.ts'
import type { DriftedSlice } from './drift-detector.ts'

export function buildLegacySyncPrompt(params: {
  repoSlug: string
  baseSha: string
  headSha: string
  drifted: DriftedSlice
  commits: CommitSummary[]
}): string {
  const { repoSlug, baseSha, headSha, drifted, commits } = params
  const { slice, changedLegacyFiles, legacyDiff } = drifted

  const commitLines = commits.length > 0
    ? commits
      .map((c) => `- \`${c.shortSha}\` ${c.subject}${c.prNumber ? ` (PR #${c.prNumber})` : ''}`)
      .join('\n')
    : `- Commit range \`${baseSha.slice(0, 8)}..${headSha.slice(0, 8)}\``

  const fileList = changedLegacyFiles.length > 0
    ? changedLegacyFiles.map((file) => `- \`${file}\``).join('\n')
    : slice.legacyPaths.map((file) => `- \`${file}\` (forced audit)`).join('\n')

  return `# Task: Forward-Port Legacy Change to Rust/GPUI Slice \`${slice.id}\`

A change merged into the legacy Electron/Vue surface of **${repoSlug}** (\`${baseSha.slice(0, 8)}..${headSha.slice(0, 8)}\`) modified files belonging to already-migrated slice **\`${slice.id}\` — ${slice.name}** without updating the corresponding Rust crate **\`${slice.rustCrate}\`**.

## Target Slice Contract
- **Slice ID:** \`${slice.id}\`
- **Name:** ${slice.name}
- **Surface:** \`${slice.surface}\`
- **Feature Flag:** \`${slice.flag}\`
- **Target Rust Crate:** \`${slice.rustCrate}\`
- **Changed Legacy File(s):**
${fileList}

## Commits in Range
${commitLines}

## Legacy Unified Diff (\`${baseSha.slice(0, 8)}..${headSha.slice(0, 8)}\`)
\`\`\`diff
${legacyDiff || '# No diff captured (forced run); inspect git log on the legacy paths above.'}
\`\`\`

## Guardrails & Enterprise Constraints (MUST FOLLOW)
1. **DO NOT revert or modify legacy Vue/Electron source files.** The legacy app remains the production baseline; your job is solely to bring \`${slice.rustCrate}\` (and shared bridge/types only if strictly required) into behavioral parity.
2. **DO NOT flip \`prod\` defaults in \`packages/desktop/src/shared/flags/environments.json\`.** Production stays locked to \`legacy\` for human-gated rollout.
3. **Read \`docs/migration/MEMORY.md\` and \`.cursor/rules/rust-gpui-panes.mdc\` first** so you respect known GPUI/WASM/serde invariants (e.g., no JS \`Map\` serialization traps, theme & IPC contracts).
4. **If the legacy change is purely Vue-internal refactoring with zero user-visible or IPC/persistence behavior delta**, make no spurious edits — clearly explain why no Rust port is required.
5. **Human-in-the-loop merge gate:** Keep your changes small, well-tested, and ready for human review. Never merge your own PR.

## Execution & Verification Checklist
1. Inspect the legacy diff and surrounding legacy code to understand the exact user-facing or state/persistence behavior change.
2. Implement the matching behavior inside \`${slice.rustCrate}\` (and \`rust/crates/marktext-wasm-host\` / \`rust/crates/mt-pref-app\` only if the host bridge payload changed).
3. Add or update unit tests in \`${slice.rustCrate}\` covering the forward-ported behavior.
4. Run verification commands and record exact pass/fail output in the PR summary:
   - \`cargo test --manifest-path rust/Cargo.toml -p ${slice.rustCrate.split('/').pop() ?? ''}\`
   - \`cargo clippy --manifest-path rust/Cargo.toml -p ${slice.rustCrate.split('/').pop() ?? ''} --all-targets -- -D warnings\`
5. In your final response / PR body, include:
   - What changed in legacy (\`${baseSha.slice(0, 8)}..${headSha.slice(0, 8)}\`)
   - What was forward-ported into \`${slice.rustCrate}\`
   - Test & clippy verification output
   - Rollback note (re-confirming \`${slice.flag}=legacy\` remains unaffected)
`
}
