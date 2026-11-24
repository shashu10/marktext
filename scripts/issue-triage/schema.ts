export type TriageConfidence = 'high' | 'medium' | 'low'

export interface SuspectCommit {
  sha: string
  prNumber: number | null
  reason: string
}

export interface TriageAnalysis {
  summary: string
  affectedSlices: string[]
  suspectedFiles: string[]
  suspectCommits: SuspectCommit[]
  rootCauseHypothesis: string
  reproHint: string
  suggestedNextStep: string
  confidence: TriageConfidence
  evidence: string[]
}

export const VALID_CONFIDENCE = new Set<TriageConfidence>(['high', 'medium', 'low'])

const TRIAGE_MARKER_RE = /<!--\s*issue-triage-agent:v1\b[^>]*-->/i

export function buildTriageMarker(issueNumber: number, headSha: string): string {
  return `<!-- issue-triage-agent:v1 issue:${issueNumber} sha:${headSha.slice(0, 10)} -->`
}

export function hasTriageMarker(body: string): boolean {
  return TRIAGE_MARKER_RE.test(body)
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

function normalizeSha(raw: string): string {
  const cleaned = raw.trim().replace(/[^a-fA-F0-9]/g, '')
  return cleaned.slice(0, 40)
}

export function validateTriageAnalysis(raw: unknown): TriageAnalysis {
  if (!raw || typeof raw !== 'object') {
    throw new Error('Triage analysis must be a JSON object')
  }
  const obj = raw as Record<string, unknown>

  const summary = asString(obj.summary)
  if (!summary) {
    throw new Error('Triage analysis is missing a non-empty summary')
  }

  const rootCauseHypothesis = asString(obj.rootCauseHypothesis)
  if (!rootCauseHypothesis) {
    throw new Error('Triage analysis is missing a non-empty rootCauseHypothesis')
  }

  const confidenceRaw = asString(obj.confidence, 'medium').toLowerCase() as TriageConfidence
  if (!VALID_CONFIDENCE.has(confidenceRaw)) {
    throw new Error(
      `Triage analysis has invalid confidence "${String(obj.confidence)}" (expected high|medium|low)`
    )
  }

  const rawCommits = Array.isArray(obj.suspectCommits) ? obj.suspectCommits : []
  const suspectCommits: SuspectCommit[] = rawCommits
    .filter((entry): entry is Record<string, unknown> => Boolean(entry) && typeof entry === 'object')
    .map((entry) => {
      const sha = normalizeSha(asString(entry.sha))
      const prRaw = entry.prNumber === null || entry.prNumber === undefined ? null : Number(entry.prNumber)
      const prNumber = prRaw !== null && Number.isInteger(prRaw) && prRaw > 0 ? prRaw : null
      const reason = asString(entry.reason, 'Touched relevant code paths.')
      return { sha, prNumber, reason }
    })
    .filter((entry) => entry.sha.length >= 7)

  return {
    summary,
    affectedSlices: [...new Set(asStringArray(obj.affectedSlices))],
    suspectedFiles: [...new Set(asStringArray(obj.suspectedFiles))],
    suspectCommits,
    rootCauseHypothesis,
    reproHint: asString(obj.reproHint, 'Reproduce using the steps listed in the issue body.'),
    suggestedNextStep: asString(
      obj.suggestedNextStep,
      'Assign to the routed owner team to verify root cause and prepare a fix PR.'
    ),
    confidence: confidenceRaw,
    evidence: asStringArray(obj.evidence)
  }
}

export function parseTriageAnalysisFromText(agentOutput: string): TriageAnalysis {
  const taggedMatch = agentOutput.match(/```triage-report-json\s*\n([\s\S]*?)```/i)
  if (taggedMatch?.[1]) {
    return validateTriageAnalysis(JSON.parse(taggedMatch[1].trim()))
  }

  const jsonBlocks = [...agentOutput.matchAll(/```json\s*\n([\s\S]*?)```/gi)]
  for (let i = jsonBlocks.length - 1; i >= 0; i--) {
    const candidate = jsonBlocks[i]?.[1]?.trim()
    if (!candidate) continue
    try {
      const parsed = JSON.parse(candidate) as Record<string, unknown>
      if (parsed && typeof parsed === 'object' && 'rootCauseHypothesis' in parsed) {
        return validateTriageAnalysis(parsed)
      }
    } catch {
      // Keep scanning earlier blocks.
    }
  }

  const trimmed = agentOutput.trim()
  if (trimmed.startsWith('{') && trimmed.endsWith('}')) {
    return validateTriageAnalysis(JSON.parse(trimmed))
  }

  throw new Error('Could not locate a valid ```triage-report-json block in the triage agent output.')
}
