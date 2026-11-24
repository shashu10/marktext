import { execFileSync } from 'node:child_process'
import { ensureGitHubLabels } from '../qa-monitor/issue-filer.ts'
import {
  buildRoutingLabelSpecs,
  isTriageEligibleLabels,
  type ResolvedOwnerRoute
} from './ownership.ts'
import type { GitHubIssueData } from './prompt.ts'
import {
  buildTriageMarker,
  hasTriageMarker,
  type TriageAnalysis
} from './schema.ts'

export interface TriageCommentMeta {
  repoSlug: string
  headSha: string
  issueNumber: number
  agentId: string | null
  runId: string | null
  runtime: 'cloud' | 'local'
  dryRun: boolean
}

export interface PostedTriageRecord {
  status: 'commented' | 'updated' | 'dry-run'
  commentId?: number
  commentUrl?: string
  appliedLabels: string[]
  route: ResolvedOwnerRoute
}

interface RawGitHubIssue {
  number: number
  title: string
  body: string | null
  html_url: string
  state: string
  pull_request?: object
  user?: { login?: string } | null
  labels?: Array<{ name?: string }>
}

interface RawGitHubComment {
  id: number
  body: string | null
  html_url: string
}

function runGh(args: string[], repoRoot: string): string {
  return execFileSync('gh', args, {
    cwd: repoRoot,
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
    maxBuffer: 32 * 1024 * 1024
  }).trim()
}

export function fetchIssueByNumber(
  repoRoot: string,
  repoSlug: string,
  issueNumber: number
): GitHubIssueData {
  const raw = runGh(['api', `repos/${repoSlug}/issues/${issueNumber}`], repoRoot)
  const item = JSON.parse(raw) as RawGitHubIssue
  if (item.pull_request) {
    throw new Error(`#${issueNumber} is a pull request, not an issue.`)
  }
  return {
    number: item.number,
    title: item.title,
    body: item.body ?? '',
    labels: (item.labels ?? []).map((l) => l.name ?? '').filter(Boolean),
    author: item.user?.login ?? 'unknown',
    url: item.html_url,
    state: item.state
  }
}

export function listIssueComments(
  repoRoot: string,
  repoSlug: string,
  issueNumber: number
): Array<{ id: number; body: string; url: string }> {
  const raw = runGh([
    'api',
    '--paginate',
    '--slurp',
    `repos/${repoSlug}/issues/${issueNumber}/comments?per_page=100`
  ], repoRoot)
  const pages = JSON.parse(raw) as RawGitHubComment[][]
  return pages.flat().map((comment) => ({
    id: comment.id,
    body: comment.body ?? '',
    url: comment.html_url
  }))
}

export function findExistingTriageComment(
  comments: Array<{ id: number; body: string; url: string }>
): { id: number; body: string; url: string } | undefined {
  return comments.find((comment) => hasTriageMarker(comment.body))
}

export function listUntriagedBugIssueNumbers(
  repoRoot: string,
  repoSlug: string,
  limit = 5
): number[] {
  const raw = runGh([
    'api',
    '--paginate',
    '--slurp',
    `repos/${repoSlug}/issues?state=open&per_page=100`
  ], repoRoot)
  const pages = JSON.parse(raw) as RawGitHubIssue[][]
  const candidates = pages
    .flat()
    .filter((item) => !item.pull_request)
    .filter((item) => {
      const labels = (item.labels ?? []).map((l) => l.name ?? '').filter(Boolean)
      return isTriageEligibleLabels(labels).eligible
    })

  const untriaged: number[] = []
  for (const issue of candidates) {
    if (untriaged.length >= limit) break
    const comments = listIssueComments(repoRoot, repoSlug, issue.number)
    if (!findExistingTriageComment(comments)) {
      untriaged.push(issue.number)
    }
  }
  return untriaged
}

export function renderTriageComment(
  analysis: TriageAnalysis,
  route: ResolvedOwnerRoute,
  meta: TriageCommentMeta
): string {
  const marker = buildTriageMarker(meta.issueNumber, meta.headSha)
  const sliceList = route.matchedSlices.length > 0
    ? route.matchedSlices.map((s) => `\`${s.id}\` (${s.name})`).join(', ')
    : analysis.affectedSlices.length > 0
      ? analysis.affectedSlices.map((s) => `\`${s}\``).join(', ')
      : '_Unmapped / cross-cutting_'

  const ownerValue = route.demoMode
    ? `\`${route.team}\` (${route.area}) · Demo assignee: \`@${route.demoAssignee}\``
    : `\`${route.team}\` (${route.area})`

  const commitsSection = analysis.suspectCommits.length > 0
    ? [
      '| Commit | PR | Why It Is Suspect |',
      '| --- | --- | --- |',
      ...analysis.suspectCommits.map((commit) => {
        const shortSha = commit.sha.slice(0, 8)
        const shaLink = `[\`${shortSha}\`](https://github.com/${meta.repoSlug}/commit/${commit.sha})`
        const prCell = commit.prNumber ? `#${commit.prNumber}` : '—'
        return `| ${shaLink} | ${prCell} | ${commit.reason.replace(/\|/g, '\\|')} |`
      })
    ].join('\n')
    : '_No single introducing commit could be isolated from git history with confidence._'

  const filesSection = analysis.suspectedFiles.length > 0
    ? analysis.suspectedFiles.map((file) => `- \`${file}\``).join('\n')
    : '- _No specific source files isolated yet._'

  const evidenceSection = analysis.evidence.length > 0
    ? analysis.evidence.map((item) => `- ${item}`).join('\n')
    : '- Derived from repository source and git history inspection.'

  const runRef = meta.agentId
    ? `\`${meta.agentId}\` (run \`${meta.runId ?? 'n/a'}\`, runtime: \`${meta.runtime}\`, commit \`${meta.headSha.slice(0, 8)}\`)`
    : `runtime: \`${meta.runtime}\`, commit \`${meta.headSha.slice(0, 8)}\``

  const lines = [
    '## Automated Triage & Root-Cause Attribution',
    '',
    analysis.summary,
    '',
    '| Field | Value |',
    '| --- | --- |',
    `| **Routed Owner** | ${ownerValue} |`,
    `| **Routing Rule** | \`${route.matchedBy}:${route.matchedKey}\` via \`docs/migration/OWNERS.json\` (${route.escalationChannel}) |`,
    `| **Affected Slice(s)** | ${sliceList} |`,
    `| **Confidence** | \`${analysis.confidence}\` |`,
    `| **Triage Agent Run** | ${runRef} |`,
    '',
    '### Root-Cause Hypothesis',
    analysis.rootCauseHypothesis,
    '',
    '### Suspect Changes',
    commitsSection,
    '',
    '### Suspected Files',
    filesSection,
    '',
    '### Verification & Suggested Next Step',
    `- **Repro / Verify:** ${analysis.reproHint}`,
    `- **Next Step:** ${analysis.suggestedNextStep}`,
    '',
    '### Evidence',
    evidenceSection
  ]

  if (route.demoMode) {
    lines.push('', `> _Note: ${route.demoModeNote}_`)
  }

  lines.push(
    '',
    '---',
    '_Posted by the decoupled Issue Triage SDK workflow (read-only analysis — no code changes made). Add `triage:skip` to opt out._',
    '',
    marker
  )

  return lines.join('\n')
}

export function postTriageResult(params: {
  repoRoot: string
  analysis: TriageAnalysis
  route: ResolvedOwnerRoute
  meta: TriageCommentMeta
}): PostedTriageRecord {
  const { repoRoot, analysis, route, meta } = params
  const labelSpecs = buildRoutingLabelSpecs(route)
  const labelNames = labelSpecs.map((spec) => spec.name)
  const body = renderTriageComment(analysis, route, meta)

  if (meta.dryRun) {
    return {
      status: 'dry-run',
      appliedLabels: labelNames,
      route
    }
  }

  ensureGitHubLabels(repoRoot, meta.repoSlug, labelSpecs, false)

  const existingComments = listIssueComments(repoRoot, meta.repoSlug, meta.issueNumber)
  const existing = findExistingTriageComment(existingComments)

  let commentId: number | undefined
  let commentUrl: string | undefined
  let status: 'commented' | 'updated'

  if (existing) {
    const raw = runGh([
      'api',
      '-X',
      'PATCH',
      `repos/${meta.repoSlug}/issues/comments/${existing.id}`,
      '-f',
      `body=${body}`
    ], repoRoot)
    const updated = JSON.parse(raw) as RawGitHubComment
    commentId = updated.id
    commentUrl = updated.html_url
    status = 'updated'
  } else {
    const raw = runGh([
      'api',
      '-X',
      'POST',
      `repos/${meta.repoSlug}/issues/${meta.issueNumber}/comments`,
      '-f',
      `body=${body}`
    ], repoRoot)
    const created = JSON.parse(raw) as RawGitHubComment
    commentId = created.id
    commentUrl = created.html_url
    status = 'commented'
  }

  // Apply deterministic routing labels without touching existing issue labels.
  const labelArgs = [
    'issue',
    'edit',
    String(meta.issueNumber),
    '--repo',
    meta.repoSlug
  ]
  for (const labelName of labelNames) {
    labelArgs.push('--add-label', labelName)
  }
  runGh(labelArgs, repoRoot)

  return {
    status,
    commentId,
    commentUrl,
    appliedLabels: labelNames,
    route
  }
}
