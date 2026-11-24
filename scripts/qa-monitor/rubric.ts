export type SeverityLevel = 'P0' | 'P1' | 'P2' | 'P3'

export type RegressionCategory =
  | 'blank-or-crash'
  | 'data-or-rollback'
  | 'interaction-or-scroll'
  | 'performance'
  | 'component-divergence'
  | 'copy-or-layout'
  | 'visual-polish'

export interface SeveritySpec {
  level: SeverityLevel
  label: `severity:${SeverityLevel}`
  color: string
  title: string
  description: string
  examples: string[]
}

export interface LabelSpec {
  name: string
  color: string
  description: string
}

export interface QaFinding {
  title: string
  severity: SeverityLevel
  slice: string
  category: RegressionCategory
  fingerprint: string
  summary: string
  expected: string
  actual: string
  reproSteps: string[]
  workaround: string | null
  rubricJustification: string
  evidence: string[]
  relatedPrs: number[]
}

export interface QaAuditReport {
  summary: string
  slicesAudited: string[]
  findings: QaFinding[]
}

export const SEVERITY_RUBRIC: Record<SeverityLevel, SeveritySpec> = {
  P0: {
    level: 'P0',
    label: 'severity:P0',
    color: 'b60205',
    title: 'Critical / Blocker',
    description: 'Feature is completely broken, unusable, crashes, or breaks persistence/rollback.',
    examples: [
      'Black or blank screen where the migrated view should render',
      'Application crash, hang, or WASM initialization failure',
      'Preference edits fail to persist to preferences.json or broadcast to open windows',
      'Rollback via flag=legacy fails to restore the legacy Vue view'
    ]
  },
  P1: {
    level: 'P1',
    label: 'severity:P1',
    color: 'd93f0b',
    title: 'High — Functional Regression With Workaround',
    description: 'Major functional or interaction failure where a user can still work around the issue.',
    examples: [
      'User cannot scroll a settings pane or panel (workaround: resize the window)',
      'Keyboard navigation or shortcut recording broken while mouse still works',
      'Required control missing from the migrated view',
      'Severe startup, pane-switch, memory, or idle CPU regression (>2x baseline)'
    ]
  },
  P2: {
    level: 'P2',
    label: 'severity:P2',
    color: 'fbca04',
    title: 'Medium-Low — UX / Control Divergence (Usable)',
    description: 'Feature works end-to-end and remains usable, but control type or UX noticeably diverged from legacy.',
    examples: [
      'Dropdown component changed into a list of buttons (user can still select all options)',
      'Missing helper note, tooltip, or secondary description',
      'Noticeable section ordering or layout structure change without lost functionality'
    ]
  },
  P3: {
    level: 'P3',
    label: 'severity:P3',
    color: '0e8a16',
    title: 'Lowest — Cosmetic / Visual Polish',
    description: 'Purely aesthetic micro-differences with zero functional impact.',
    examples: [
      'Button border-radius differs from the legacy component',
      'Font spacing, kerning, or line-height differs slightly',
      'Minor padding, margin, divider, or color shade mismatch'
    ]
  }
}

export const BASE_LABELS: LabelSpec[] = [
  {
    name: 'regression',
    color: 'e11d48',
    description: 'Confirmed regression detected during migration QA'
  },
  {
    name: 'qa-triage',
    color: '5319e7',
    description: 'Filed automatically by the Migration QA Monitor for triage'
  },
  ...Object.values(SEVERITY_RUBRIC).map((spec) => ({
    name: spec.label,
    color: spec.color,
    description: spec.title
  }))
]

export const VALID_SEVERITIES = new Set<SeverityLevel>(['P0', 'P1', 'P2', 'P3'])

export const VALID_CATEGORIES = new Set<RegressionCategory>([
  'blank-or-crash',
  'data-or-rollback',
  'interaction-or-scroll',
  'performance',
  'component-divergence',
  'copy-or-layout',
  'visual-polish'
])

export function normalizeFingerprint(slice: string, raw: string): string {
  const slicePrefix = `${slice.toLowerCase()}:`
  const input = raw.trim().toLowerCase()
  const slug = input.startsWith(slicePrefix) ? input.slice(slicePrefix.length) : input
  const cleaned = slug
    .replace(/[^a-z0-9:_-]+/g, '-')
    .replace(/-{2,}/g, '-')
    .replace(/^-|-$/g, '')
  return `${slicePrefix}${cleaned || 'unnamed-regression'}`
}

export function sliceLabel(slice: string): LabelSpec {
  return {
    name: `slice:${slice}`,
    color: '1d76db',
    description: `Migration slice ${slice}`
  }
}

export function severityRank(level: SeverityLevel): number {
  switch (level) {
    case 'P0': return 0
    case 'P1': return 1
    case 'P2': return 2
    case 'P3': return 3
  }
}

function asString(value: unknown, fallback = ''): string {
  return typeof value === 'string' ? value.trim() : fallback
}

function asStringArray(value: unknown): string[] {
  if (!Array.isArray(value)) return []
  return value
    .filter((item): item is string => typeof item === 'string')
    .map((item) => item.trim())
    .filter(Boolean)
}

function asNumberArray(value: unknown): number[] {
  if (!Array.isArray(value)) return []
  return value
    .map((item) => (typeof item === 'number' ? item : Number(item)))
    .filter((num) => Number.isInteger(num) && num > 0)
}

export function validateAuditReport(raw: unknown): QaAuditReport {
  if (!raw || typeof raw !== 'object') {
    throw new Error('QA audit report must be a JSON object')
  }
  const obj = raw as Record<string, unknown>
  const summary = asString(obj.summary, 'QA regression audit completed.')
  const slicesAudited = asStringArray(obj.slicesAudited)
  if (!Array.isArray(obj.findings)) {
    throw new Error('QA audit report must include a findings array, even for a clean audit')
  }

  const findings: QaFinding[] = obj.findings.map((entry, idx) => {
    if (!entry || typeof entry !== 'object') {
      throw new Error(`Finding at index ${idx} must be an object`)
    }
    const item = entry as Record<string, unknown>
    const severityRaw = asString(item.severity).toUpperCase() as SeverityLevel
    if (!VALID_SEVERITIES.has(severityRaw)) {
      throw new Error(`Finding at index ${idx} has invalid severity "${String(item.severity)}" (expected P0-P3)`)
    }

    const categoryRaw = asString(item.category, 'visual-polish') as RegressionCategory
    const category: RegressionCategory = VALID_CATEGORIES.has(categoryRaw)
      ? categoryRaw
      : 'visual-polish'

    const title = asString(item.title)
    if (!title) {
      throw new Error(`Finding at index ${idx} is missing a non-empty title`)
    }

    const slice = asString(item.slice, slicesAudited[0] || 'general')
    const rawFingerprint = asString(item.fingerprint, title)
    const workaroundRaw = item.workaround === null ? null : asString(item.workaround)

    return {
      title,
      severity: severityRaw,
      slice,
      category,
      fingerprint: normalizeFingerprint(slice, rawFingerprint),
      summary: asString(item.summary, title),
      expected: asString(item.expected, 'Matches legacy behavior and visual specification.'),
      actual: asString(item.actual, 'Observed regression in migrated view.'),
      reproSteps: asStringArray(item.reproSteps),
      workaround: workaroundRaw || null,
      rubricJustification: asString(
        item.rubricJustification,
        `Classified as ${severityRaw} per ${SEVERITY_RUBRIC[severityRaw].title}.`
      ),
      evidence: asStringArray(item.evidence),
      relatedPrs: asNumberArray(item.relatedPrs)
    }
  })

  findings.sort((a, b) => severityRank(a.severity) - severityRank(b.severity))
  return { summary, slicesAudited, findings }
}

/**
 * Extracts and validates the JSON audit report emitted by the QA/TPM agent.
 * Prefers a fenced ```qa-findings-json block, then falls back to ```json or raw JSON.
 */
export function parseAuditReportFromText(agentOutput: string): QaAuditReport {
  const taggedMatch = agentOutput.match(/```qa-findings-json\s*\n([\s\S]*?)```/i)
  if (taggedMatch?.[1]) {
    return validateAuditReport(JSON.parse(taggedMatch[1].trim()))
  }

  const jsonBlocks = [...agentOutput.matchAll(/```json\s*\n([\s\S]*?)```/gi)]
  for (let i = jsonBlocks.length - 1; i >= 0; i--) {
    const candidate = jsonBlocks[i]?.[1]?.trim()
    if (!candidate) continue
    try {
      const parsed = JSON.parse(candidate) as Record<string, unknown>
      if (parsed && typeof parsed === 'object' && 'findings' in parsed) {
        return validateAuditReport(parsed)
      }
    } catch {
      // Keep scanning previous JSON blocks.
    }
  }

  const trimmed = agentOutput.trim()
  if (trimmed.startsWith('{') && trimmed.endsWith('}')) {
    return validateAuditReport(JSON.parse(trimmed))
  }

  throw new Error(
    'Could not locate a valid ```qa-findings-json block in the QA agent output.'
  )
}

export function formatRubricMarkdown(): string {
  return Object.values(SEVERITY_RUBRIC)
    .map((spec) => {
      const examples = spec.examples.map((ex) => `  - ${ex}`).join('\n')
      return `### ${spec.level} (\`${spec.label}\`) — ${spec.title}\n${spec.description}\nExamples:\n${examples}`
    })
    .join('\n\n')
}
