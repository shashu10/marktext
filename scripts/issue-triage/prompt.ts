import type { CommitSummary, SliceMetadata } from '../qa-monitor/change-detector.ts'

export interface GitHubIssueData {
  number: number
  title: string
  body: string
  labels: string[]
  author: string
  url: string
  state: string
}

export function buildTriagePrompt(params: {
  repoSlug: string
  headSha: string
  issue: GitHubIssueData
  slices: SliceMetadata[]
  recentCommits: CommitSummary[]
}): string {
  const { repoSlug, headSha, issue, slices, recentCommits } = params

  const sliceCatalogBlock = slices
    .map(
      (slice) =>
        `- **${slice.id}** (${slice.name}, surface \`${slice.surface}\`, flag \`${slice.flag}\`): rust=\`${slice.rustCrate}\`, legacy=\`${slice.legacyPaths.join(', ')}\``
    )
    .join('\n')

  const recentCommitsBlock = recentCommits.length > 0
    ? recentCommits
      .slice(0, 30)
      .map((c) => `- \`${c.shortSha}\` ${c.subject}${c.prNumber ? ` (PR #${c.prNumber})` : ''} — ${c.author} (${c.date})`)
      .join('\n')
    : '- (Use `git log -n 30 --oneline` locally to inspect recent commits.)'

  const labelList = issue.labels.length > 0 ? issue.labels.map((l) => `\`${l}\``).join(', ') : '_none_'
  const boundedBody = issue.body.length > 8000
    ? `${issue.body.slice(0, 8000)}\n\n...[truncated for prompt budget]`
    : issue.body

  return `# Role: Independent Bug Triage & Root-Cause Attribution Agent

You are triaging a bug/regression issue filed on **${repoSlug}** at commit \`${headSha}\`.
This issue may have been filed either by a human contributor or by the separate Migration QA Monitor.

## STRICT NON-MODIFYING CHARTER (DO NOT VIOLATE)
1. **DO NOT FIX THE BUG.** You are strictly read-only. Never edit source files, never commit, never push branches, and never open PRs.
2. **DO NOT INVENT COMMIT SHAS OR FILE PATHS.** Every commit SHA, PR number, and file path you cite MUST be verified via \`git log\`, \`git show\`, \`git blame\`, or reading files in this repository.
3. **DO NOT INVENT OWNER HANDLES.** Ownership routing is computed deterministically downstream from \`docs/migration/OWNERS.json\` using the \`affectedSlices\` and \`suspectedFiles\` you identify. Focus on accurate technical attribution.

## Target GitHub Issue
- **Issue:** #${issue.number} — ${issue.title}
- **URL:** ${issue.url}
- **Author:** @${issue.author}
- **Labels:** ${labelList}

### Issue Description
${boundedBody || '_No issue description provided._'}

## Known Migration Slices (\`docs/migration/slices.json\`)
${sliceCatalogBlock}

## Recent Repository Commits (Starting Context)
${recentCommitsBlock}

## Investigation Protocol
1. Read \`docs/migration/MEMORY.md\` and \`docs/migration/STATUS.md\` for known architectural invariants and past migration pitfalls.
2. Identify which slice(s) and source files (\`rust/crates/...\`, \`packages/desktop/src/...\`, \`packages/muya/src/...\`) implement the behavior described in the issue.
3. Run targeted read-only git forensics (\`git log -n 10 -- <path>\`, \`git show --stat <sha>\`, \`git blame\`) to pinpoint the most likely introducing commit(s) and PR(s).
4. Inspect the suspected source files to form a concrete, evidence-backed root-cause hypothesis and a concise next step for the owning engineer.
5. Be honest about confidence: use \`high\` only when code + git history directly prove the mechanism, \`medium\` when strongly indicated, and \`low\` when reproduction or runtime logs are still needed.

## Required Final Output Format
End your response with a single fenced \`\`\`triage-report-json block matching this exact JSON schema:

\`\`\`triage-report-json
{
  "summary": "One or two sentence executive triage summary.",
  "affectedSlices": ["A1", "A2"],
  "suspectedFiles": ["rust/crates/marktext-wasm-host/src/lib.rs"],
  "suspectCommits": [
    {
      "sha": "4b3f3c0f",
      "prNumber": 20,
      "reason": "Why this commit/PR is likely responsible or most closely related."
    }
  ],
  "rootCauseHypothesis": "Concrete technical explanation citing symbols, functions, or state flow verified in the code.",
  "reproHint": "Minimal command or user action to verify the bug locally.",
  "suggestedNextStep": "Actionable engineering handoff step for the owning team.",
  "confidence": "high",
  "evidence": [
    "File path + line/symbol or git commit observation backing the hypothesis."
  ]
}
\`\`\`
`
}
