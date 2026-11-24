import { execFileSync } from 'node:child_process'
import {
  BASE_LABELS,
  SEVERITY_RUBRIC,
  sliceLabel,
  type LabelSpec,
  type QaAuditReport,
  type QaFinding
} from './rubric.ts'

export interface ExistingRegressionIssue {
  number: number
  title: string
  url: string
  fingerprint: string | null
}

export interface FiledIssueRecord {
  finding: QaFinding
  status: 'filed' | 'dry-run' | 'skipped-duplicate'
  issueNumber?: number
  issueUrl?: string
  duplicateOf?: number
}

export interface IssueFilerMetadata {
  repoSlug: string
  baseSha: string | null
  headSha: string
  agentId: string | null
  runId: string | null
  runtime: 'cloud' | 'local'
  dryRun: boolean
}

const FINGERPRINT_RE = /<!--\s*qa-fingerprint:\s*([a-z0-9:._-]+)\s*-->/i

export function extractFingerprintFromBody(body: string): string | null {
  const match = body.match(FINGERPRINT_RE)
  return match?.[1]?.toLowerCase() ?? null
}

export function renderIssueTitle(finding: QaFinding): string {
  return `[${finding.severity}][${finding.slice}] ${finding.title}`
}

export function renderIssueBody(
  finding: QaFinding,
  meta: IssueFilerMetadata
): string {
  const severity = SEVERITY_RUBRIC[finding.severity]
  const commitRange = meta.baseSha
    ? `\`${meta.baseSha.slice(0, 10)}..${meta.headSha.slice(0, 10)}\``
    : `\`${meta.headSha.slice(0, 10)}\``

  const prLinks = finding.relatedPrs.length > 0
    ? finding.relatedPrs.map((num) => `#${num}`).join(', ')
    : '_Not attributed to a specific PR_'

  const reproList = finding.reproSteps.length > 0
    ? finding.reproSteps.map((step, i) => `${i + 1}. ${step}`).join('\n')
    : '1. Launch MarkText with the slice feature flag enabled (`rust-wasm`).\n2. Open the affected view and compare against `flag=legacy`.'

  const evidenceList = finding.evidence.length > 0
    ? finding.evidence.map((item) => `- ${item}`).join('\n')
    : '- Observed during automated Before/After visual and functional parity audit.'

  const runRef = meta.agentId
    ? `\`${meta.agentId}\` (run \`${meta.runId ?? 'n/a'}\`, runtime: \`${meta.runtime}\`)`
    : `runtime: \`${meta.runtime}\``

  return [
    `## Regression Summary`,
    '',
    finding.summary,
    '',
    '| Field | Value |',
    '| --- | --- |',
    `| **Severity** | \`${severity.label}\` — ${severity.title} |`,
    `| **Slice** | \`${finding.slice}\` |`,
    `| **Category** | \`${finding.category}\` |`,
    `| **Audited Range** | ${commitRange} |`,
    `| **Related PR(s)** | ${prLinks} |`,
    `| **Workaround** | ${finding.workaround ?? 'None identified'} |`,
    `| **QA Agent Run** | ${runRef} |`,
    '',
    '### Expected (Legacy Electron/Vue)',
    finding.expected,
    '',
    '### Actual (Migrated Rust/GPUI)',
    finding.actual,
    '',
    '### Steps to Reproduce',
    reproList,
    '',
    '### Evidence & Telemetry',
    evidenceList,
    '',
    '### Severity Rubric Justification',
    `> ${finding.rubricJustification}`,
    '',
    '---',
    '_Filed automatically by the Migration QA Monitor (`qa-triage`, read-only — no code changes made). See `docs/migration/QA_RUBRIC.md` for severity definitions._',
    '',
    `<!-- qa-fingerprint: ${finding.fingerprint} -->`
  ].join('\n')
}

export function labelsForFinding(finding: QaFinding): string[] {
  const labels = [
    SEVERITY_RUBRIC[finding.severity].label,
    'regression',
    'qa-triage'
  ]
  if (finding.slice) {
    labels.push(sliceLabel(finding.slice).name)
  }
  return labels
}

function runGh(args: string[], repoRoot: string): string {
  return execFileSync('gh', args, {
    cwd: repoRoot,
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
    maxBuffer: 32 * 1024 * 1024
  }).trim()
}

export function listOpenRegressionIssues(
  repoRoot: string,
  repoSlug: string,
  required = false
): ExistingRegressionIssue[] {
  try {
    const raw = runGh([
      'api',
      '--paginate',
      '--slurp',
      `repos/${repoSlug}/issues?state=open&labels=regression&per_page=100`
    ], repoRoot)
    const pages = JSON.parse(raw) as Array<Array<{
      number: number
      title: string
      html_url: string
      body: string | null
      pull_request?: object
    }>>
    return pages.flat().filter(item => !item.pull_request).map((item) => ({
      number: item.number,
      title: item.title,
      url: item.html_url,
      fingerprint: extractFingerprintFromBody(item.body ?? '')
    }))
  } catch (err) {
    if (required) throw err
    console.warn(
      `[qa-monitor] Warning: could not fetch open regression issues (${err instanceof Error ? err.message : String(err)})`
    )
    return []
  }
}

export function ensureGitHubLabels(
  repoRoot: string,
  repoSlug: string,
  labels: LabelSpec[],
  dryRun: boolean
): void {
  if (dryRun) return
  for (const label of labels) {
    try {
      runGh(
        [
          'label',
          'create',
          label.name,
          '--repo',
          repoSlug,
          '--color',
          label.color,
          '--description',
          label.description,
          '--force'
        ],
        repoRoot
      )
    } catch (err) {
      console.warn(
        `[qa-monitor] Warning: could not ensure label "${label.name}" (${err instanceof Error ? err.message : String(err)})`
      )
    }
  }
}

export function fileAuditFindings(options: {
  repoRoot: string
  report: QaAuditReport
  existingIssues: ExistingRegressionIssue[]
  meta: IssueFilerMetadata
}): FiledIssueRecord[] {
  const { repoRoot, report, existingIssues, meta } = options
  const openByFingerprint = new Map<string, ExistingRegressionIssue>()
  for (const issue of existingIssues) {
    if (issue.fingerprint) {
      openByFingerprint.set(issue.fingerprint.toLowerCase(), issue)
    }
  }

  const requiredLabels = new Map<string, LabelSpec>()
  for (const label of BASE_LABELS) {
    requiredLabels.set(label.name, label)
  }
  for (const finding of report.findings) {
    if (finding.slice) {
      const sLabel = sliceLabel(finding.slice)
      requiredLabels.set(sLabel.name, sLabel)
    }
  }

  if (report.findings.length > 0) {
    ensureGitHubLabels(repoRoot, meta.repoSlug, [...requiredLabels.values()], meta.dryRun)
  }

  const results: FiledIssueRecord[] = []
  for (const finding of report.findings) {
    const existing = openByFingerprint.get(finding.fingerprint.toLowerCase())
    if (existing) {
      results.push({
        finding,
        status: 'skipped-duplicate',
        duplicateOf: existing.number,
        issueUrl: existing.url
      })
      continue
    }

    const title = renderIssueTitle(finding)
    const body = renderIssueBody(finding, meta)
    const labels = labelsForFinding(finding)

    if (meta.dryRun) {
      results.push({ finding, status: 'dry-run' })
      continue
    }

    const args = [
      'issue',
      'create',
      '--repo',
      meta.repoSlug,
      '--title',
      title,
      '--body',
      body
    ]
    for (const label of labels) {
      args.push('--label', label)
    }

    const issueUrl = runGh(args, repoRoot)
    const numberMatch = issueUrl.match(/\/issues\/(\d+)$/)
    const issueNumber = numberMatch?.[1] ? Number(numberMatch[1]) : undefined

    // Record newly filed fingerprint so duplicate entries within the same report are skipped too.
    openByFingerprint.set(finding.fingerprint.toLowerCase(), {
      number: issueNumber ?? 0,
      title,
      url: issueUrl,
      fingerprint: finding.fingerprint
    })

    results.push({
      finding,
      status: 'filed',
      issueNumber,
      issueUrl
    })
  }

  return results
}
