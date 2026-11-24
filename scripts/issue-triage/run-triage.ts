import { execFileSync } from 'node:child_process'
import { randomUUID } from 'node:crypto'
import { mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import type { AgentOptions, RunResult } from '@cursor/sdk'
import {
  extractPrNumberFromSubject,
  loadSlicesCatalog,
  resolveGitSha,
  type CommitSummary
} from '../qa-monitor/change-detector.ts'
import {
  fetchIssueByNumber,
  findExistingTriageComment,
  listIssueComments,
  listUntriagedBugIssueNumbers,
  postTriageResult,
  type PostedTriageRecord
} from './commenter.ts'
import {
  isTriageEligibleLabels,
  loadOwnersConfig,
  resolveOwnerRoute
} from './ownership.ts'
import { buildTriagePrompt } from './prompt.ts'
import {
  parseTriageAnalysisFromText,
  validateTriageAnalysis,
  type TriageAnalysis
} from './schema.ts'

export type TriageRuntime = 'local' | 'cloud'

export interface TriageCliOptions {
  runtime: TriageRuntime
  repoSlug: string
  headSha: string | null
  issueNumber: number | null
  findUntriaged: boolean
  limit: number
  model: string | null
  dryRun: boolean
  force: boolean
  checkOnly: boolean
  reportOnly: boolean
  outPath: string | null
  reportOutPath: string | null
  replayPath: string | null
}

export interface SavedTriageReport {
  version: 1
  startedAt: string
  finishedAt: string
  repoSlug: string
  headSha: string
  issueNumber: number
  runtime: TriageRuntime
  agentId: string
  runId: string
  analysis: TriageAnalysis
}

export interface TriageTelemetry {
  startedAt: string
  finishedAt: string
  durationMs: number
  runtime: TriageRuntime
  repoSlug: string
  headSha: string
  issueNumber: number
  skipped: boolean
  skipReason: string | null
  agentId: string | null
  runId: string | null
  runStatus: RunResult['status'] | null
  routedTeam: string | null
  matchedSlices: string[]
  suspectCommitsCount: number
  confidence: TriageAnalysis['confidence'] | null
  commentStatus: PostedTriageRecord['status'] | null
  commentUrl: string | null
  appliedLabels: string[]
  dryRun: boolean
}

const DEFAULT_REPO_SLUG = 'shashu10/marktext'
const DEFAULT_MODEL = 'composer-2.5'

export function inferRepoSlug(repoRoot: string): string {
  if (process.env.GITHUB_REPOSITORY) {
    return process.env.GITHUB_REPOSITORY
  }
  try {
    const remote = execFileSync('git', ['remote', 'get-url', 'origin'], {
      cwd: repoRoot,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe']
    }).trim()
    const match = remote.match(/github\.com[:/]([^/]+\/[^/.]+?)(?:\.git)?$/i)
    if (match?.[1]) return match[1]
  } catch {
    // Fall back to default slug.
  }
  return DEFAULT_REPO_SLUG
}

export function parseTriageCliArgs(argv: string[], repoRoot: string): TriageCliOptions {
  let runtime: TriageRuntime = 'local'
  let repoSlug = inferRepoSlug(repoRoot)
  let headSha: string | null = null
  let issueNumber: number | null = null
  let findUntriaged = false
  let limit = 5
  let model: string | null = process.env.CURSOR_TRIAGE_MODEL ?? null
  let dryRun = false
  let force = false
  let checkOnly = false
  let reportOnly = false
  let outPath: string | null = null
  let reportOutPath: string | null = null
  let replayPath: string | null = null

  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i]
    const next = (): string => {
      const value = argv[++i]
      if (!value || value.startsWith('--')) {
        throw new Error(`Missing value for ${arg}`)
      }
      return value
    }

    switch (arg) {
      case '--runtime': {
        const val = next()
        if (val !== 'local' && val !== 'cloud') {
          throw new Error(`Invalid --runtime "${val}" (expected "local" or "cloud")`)
        }
        runtime = val
        break
      }
      case '--repo':
        repoSlug = next()
        break
      case '--head-sha':
        headSha = next()
        break
      case '--issue': {
        const parsed = Number(next())
        if (!Number.isInteger(parsed) || parsed <= 0) {
          throw new Error('Invalid --issue number (expected positive integer)')
        }
        issueNumber = parsed
        break
      }
      case '--find-untriaged':
        findUntriaged = true
        break
      case '--limit': {
        const parsed = Number(next())
        if (!Number.isInteger(parsed) || parsed <= 0) {
          throw new Error('Invalid --limit value (expected positive integer)')
        }
        limit = parsed
        break
      }
      case '--model':
        model = next()
        break
      case '--out':
        outPath = next()
        break
      case '--report-only':
        reportOnly = true
        break
      case '--report-out':
        reportOutPath = next()
        break
      case '--replay':
        replayPath = next()
        break
      case '--dry-run':
        dryRun = true
        break
      case '--force':
        force = true
        break
      case '--check-only':
        checkOnly = true
        break
      case '--help':
      case '-h':
        printUsage()
        process.exit(0)
        break
      default:
        throw new Error(`Unknown argument: ${arg}`)
    }
  }

  if (!findUntriaged && !replayPath && issueNumber === null) {
    throw new Error('One of --issue <number>, --find-untriaged, or --replay <file.json> is required.')
  }
  if (reportOnly && (!reportOutPath || replayPath || checkOnly || findUntriaged)) {
    throw new Error('--report-only requires --report-out and cannot be combined with --replay, --check-only, or --find-untriaged')
  }
  if (replayPath && (reportOutPath || checkOnly || findUntriaged)) {
    throw new Error('--replay cannot be combined with --report-out, --check-only, or --find-untriaged')
  }

  return {
    runtime,
    repoSlug,
    headSha,
    issueNumber,
    findUntriaged,
    limit,
    model,
    dryRun,
    force,
    checkOnly,
    reportOnly,
    outPath,
    reportOutPath,
    replayPath
  }
}

function printUsage(): void {
  console.log(`Usage: node --no-warnings scripts/issue-triage/run-triage.ts [options]

Options:
  --issue <number>         GitHub issue number to triage
  --find-untriaged         Print JSON array of open bug/regression issue numbers lacking a triage comment
  --limit <n>              Max untriaged issues returned by --find-untriaged (default: 5)
  --runtime <local|cloud>  Cursor SDK runtime (default: local for fast git forensics)
  --repo <owner/name>      GitHub repository slug (default: inferred from origin)
  --head-sha <sha>         Commit SHA to analyze against (default: HEAD)
  --model <model-id>       Cursor model ID (default: "${DEFAULT_MODEL}")
  --check-only             Evaluate triage eligibility and idempotency gate without starting an agent
  --report-only            Save structured triage analysis to --report-out without commenting
  --report-out <file.json> Path to save the completed triage analysis JSON
  --replay <file.json>     Post/update the GitHub triage comment from a saved report (no CURSOR_API_KEY needed)
  --dry-run                Preview triage routing and comment without mutating GitHub
  --force                  Re-triage even if a triage marker comment already exists or bug label is absent
  --out <file.json>        Write structured triage telemetry JSON to disk
`)
}

export function buildTriageAgentOptions(params: {
  options: TriageCliOptions
  repoRoot: string
  headSha: string
  issueNumber: number
  apiKey: string
}): AgentOptions {
  const { options, repoRoot, headSha, issueNumber, apiKey } = params
  const shortHead = headSha.slice(0, 7)
  const modelId = options.model ?? DEFAULT_MODEL

  const base: AgentOptions = {
    apiKey,
    name: `MarkText Issue Triage #${issueNumber} (${shortHead})`,
    model: { id: modelId }
  }

  if (options.runtime === 'cloud') {
    base.cloud = {
      repos: [
        {
          url: `https://github.com/${options.repoSlug}`,
          startingRef: headSha
        }
      ],
      workOnCurrentBranch: false,
      autoCreatePR: false,
      skipReviewerRequest: true,
      metadata: {
        workflow: 'issue-triage',
        repo: options.repoSlug,
        issue_number: String(issueNumber),
        head_sha: headSha
      }
    }
    if (!options.force) {
      base.idempotencyKey = `issue-triage-${issueNumber}-${shortHead}`
    }
  } else {
    base.local = { cwd: repoRoot }
    // Hard-enforce read-only triage charter on local runtime.
    base.disallowedTools = ['edit', 'delete']
  }

  return base
}

function readRecentCommits(repoRoot: string, headSha: string, count = 30): CommitSummary[] {
  try {
    const raw = execFileSync(
      'git',
      ['log', '-n', String(count), '--format=%H%x1f%h%x1f%s%x1f%an%x1f%aI', headSha],
      { cwd: repoRoot, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }
    ).trim()
    if (!raw) return []
    return raw.split('\n').filter(Boolean).map((line) => {
      const [sha = '', shortSha = '', subject = '', author = '', date = ''] = line.split('\x1f')
      return {
        sha,
        shortSha,
        subject,
        author,
        date,
        prNumber: extractPrNumberFromSubject(subject)
      }
    })
  } catch {
    return []
  }
}

function writeJson(outPath: string, data: SavedTriageReport | TriageTelemetry): void {
  mkdirSync(dirname(outPath), { recursive: true })
  const temporary = `${outPath}.${randomUUID()}.tmp`
  writeFileSync(temporary, JSON.stringify(data, null, 2) + '\n', { mode: 0o600 })
  renameSync(temporary, outPath)
}

export function readSavedTriageReport(
  path: string,
  expectedRepoSlug: string,
  expectedIssueNumber?: number | null
): SavedTriageReport {
  const saved = JSON.parse(readFileSync(path, 'utf8')) as Partial<SavedTriageReport>
  if (
    saved?.version !== 1 ||
    saved.repoSlug !== expectedRepoSlug ||
    typeof saved.headSha !== 'string' ||
    !saved.headSha ||
    typeof saved.issueNumber !== 'number' ||
    !Number.isInteger(saved.issueNumber) ||
    saved.issueNumber <= 0 ||
    (saved.runtime !== 'local' && saved.runtime !== 'cloud') ||
    typeof saved.agentId !== 'string' ||
    !saved.agentId ||
    typeof saved.runId !== 'string' ||
    !saved.runId
  ) {
    throw new Error(`Invalid saved triage report in ${path} for ${expectedRepoSlug}`)
  }
  if (expectedIssueNumber && saved.issueNumber !== expectedIssueNumber) {
    throw new Error(
      `Saved triage report is for issue #${saved.issueNumber}, expected #${expectedIssueNumber}`
    )
  }
  return {
    version: 1,
    startedAt: saved.startedAt ?? new Date().toISOString(),
    finishedAt: saved.finishedAt ?? new Date().toISOString(),
    repoSlug: saved.repoSlug,
    headSha: saved.headSha,
    issueNumber: saved.issueNumber,
    runtime: saved.runtime,
    agentId: saved.agentId,
    runId: saved.runId,
    analysis: validateTriageAnalysis(saved.analysis)
  }
}

function publishSavedTriage(
  repoRoot: string,
  options: TriageCliOptions,
  saved: SavedTriageReport
): TriageTelemetry {
  const { slices } = loadSlicesCatalog(repoRoot)
  const owners = loadOwnersConfig(repoRoot)
  const route = resolveOwnerRoute({
    affectedSliceIds: saved.analysis.affectedSlices,
    suspectedFiles: saved.analysis.suspectedFiles,
    slicesCatalog: slices,
    owners
  })

  const posted = postTriageResult({
    repoRoot,
    analysis: saved.analysis,
    route,
    meta: {
      repoSlug: saved.repoSlug,
      headSha: saved.headSha,
      issueNumber: saved.issueNumber,
      agentId: saved.agentId,
      runId: saved.runId,
      runtime: saved.runtime,
      dryRun: options.dryRun
    }
  })

  const durationMs = Math.max(0, Date.parse(saved.finishedAt) - Date.parse(saved.startedAt))
  const telemetry: TriageTelemetry = {
    startedAt: saved.startedAt,
    finishedAt: saved.finishedAt,
    durationMs,
    runtime: saved.runtime,
    repoSlug: saved.repoSlug,
    headSha: saved.headSha,
    issueNumber: saved.issueNumber,
    skipped: false,
    skipReason: null,
    agentId: saved.agentId,
    runId: saved.runId,
    runStatus: 'finished',
    routedTeam: route.team,
    matchedSlices: route.matchedSlices.map((s) => s.id),
    suspectCommitsCount: saved.analysis.suspectCommits.length,
    confidence: saved.analysis.confidence,
    commentStatus: posted.status,
    commentUrl: posted.commentUrl ?? null,
    appliedLabels: posted.appliedLabels,
    dryRun: options.dryRun
  }

  if (options.outPath) writeJson(options.outPath, telemetry)

  console.log(`[issue-triage] === Triage Complete for #${saved.issueNumber} ===`)
  console.log(
    `[issue-triage] Routed -> ${route.team} (${route.matchedBy}:${route.matchedKey}) | ` +
    `Slices: ${telemetry.matchedSlices.join(', ') || 'none'} | ` +
    `Suspect commits: ${telemetry.suspectCommitsCount} | Confidence: ${saved.analysis.confidence} | ` +
    `Status: ${posted.status}${posted.commentUrl ? ` -> ${posted.commentUrl}` : ''}`
  )

  return telemetry
}

async function runIssueTriage(): Promise<number> {
  const startedMs = Date.now()
  const startedAt = new Date(startedMs).toISOString()
  const scriptDir = dirname(fileURLToPath(import.meta.url))
  const repoRoot = resolve(scriptDir, '../..')
  const options = parseTriageCliArgs(process.argv.slice(2), repoRoot)

  if (options.findUntriaged) {
    const issueNumbers = listUntriagedBugIssueNumbers(repoRoot, options.repoSlug, options.limit)
    console.log(JSON.stringify(issueNumbers))
    return 0
  }

  if (options.replayPath) {
    const saved = readSavedTriageReport(options.replayPath, options.repoSlug, options.issueNumber)
    console.log(
      `[issue-triage] Replaying saved triage analysis (${saved.runId}) for ${saved.repoSlug}#${saved.issueNumber}`
    )
    publishSavedTriage(repoRoot, options, saved)
    return 0
  }

  const issueNumber = options.issueNumber
  if (issueNumber === null) {
    throw new Error('Missing --issue <number>')
  }

  const headSha = options.headSha
    ? (resolveGitSha(repoRoot, options.headSha) ?? options.headSha)
    : (resolveGitSha(repoRoot, 'HEAD') ?? '')
  if (!headSha) {
    throw new Error('Could not resolve HEAD commit SHA')
  }

  const issue = fetchIssueByNumber(repoRoot, options.repoSlug, issueNumber)
  const labelCheck = isTriageEligibleLabels(issue.labels)
  const existingComments = listIssueComments(repoRoot, options.repoSlug, issueNumber)
  const existingTriage = findExistingTriageComment(existingComments)

  let shouldRun = true
  let skipReason: string | null = null

  if (!options.force) {
    if (issue.state !== 'open') {
      shouldRun = false
      skipReason = `Issue #${issueNumber} is ${issue.state}, not open.`
    } else if (!labelCheck.eligible) {
      shouldRun = false
      skipReason = labelCheck.reason
    } else if (existingTriage) {
      shouldRun = false
      skipReason = `Issue #${issueNumber} already has a triage comment (${existingTriage.url}).`
    }
  }

  if (options.checkOnly) {
    console.log(JSON.stringify({
      shouldRun,
      skipReason,
      issueNumber,
      labels: issue.labels,
      alreadyTriaged: Boolean(existingTriage),
      headSha
    }, null, 2))
    return 0
  }

  if (!shouldRun) {
    console.log(`[issue-triage] Skipping #${issueNumber}: ${skipReason}`)
    const telemetry: TriageTelemetry = {
      startedAt,
      finishedAt: new Date().toISOString(),
      durationMs: Date.now() - startedMs,
      runtime: options.runtime,
      repoSlug: options.repoSlug,
      headSha,
      issueNumber,
      skipped: true,
      skipReason,
      agentId: null,
      runId: null,
      runStatus: null,
      routedTeam: null,
      matchedSlices: [],
      suspectCommitsCount: 0,
      confidence: null,
      commentStatus: null,
      commentUrl: null,
      appliedLabels: [],
      dryRun: options.dryRun
    }
    if (options.outPath) writeJson(options.outPath, telemetry)
    return 0
  }

  const apiKey = process.env.CURSOR_API_KEY
  if (!apiKey) {
    console.error('[issue-triage] Error: CURSOR_API_KEY environment variable is required.')
    return 1
  }

  const { slices } = loadSlicesCatalog(repoRoot)
  const recentCommits = readRecentCommits(repoRoot, headSha)
  const prompt = buildTriagePrompt({
    repoSlug: options.repoSlug,
    headSha,
    issue,
    slices,
    recentCommits
  })

  console.log(
    `[issue-triage] Starting ${options.runtime} triage for ${options.repoSlug}#${issueNumber} at ${headSha.slice(0, 8)}`
  )

  const agentOptions = buildTriageAgentOptions({
    options,
    repoRoot,
    headSha,
    issueNumber,
    apiKey
  })
  const { Agent, CursorAgentError } = await import('@cursor/sdk')
  let agent: Awaited<ReturnType<typeof Agent.create>> | null = null
  let agentId: string | null
  let runId: string | null
  let result: RunResult | null

  try {
    agent = await Agent.create(agentOptions)
    agentId = agent.agentId
    console.log(`[issue-triage] Agent created: agentId=${agentId} (runtime=${options.runtime})`)

    const run = await agent.send(prompt)
    runId = run.id
    console.log(`[issue-triage] Run started: runId=${runId}`)

    for await (const event of run.stream()) {
      if (event.type === 'status') {
        const msg = event.message ? ` (${event.message})` : ''
        console.log(`[issue-triage] status: ${event.status}${msg}`)
      } else if (event.type === 'tool_call' && event.status === 'running') {
        console.log(`[issue-triage] tool: ${event.name}`)
      } else if (event.type === 'summary') {
        console.log(`[issue-triage] summary: ${event.status}`)
      }
    }

    result = await run.wait()
  } catch (err) {
    if (err instanceof CursorAgentError) {
      console.error(
        `[issue-triage] CursorAgentError (code=${err.code ?? 'unknown'}, retryable=${String(err.isRetryable)}): ${err.message}`
      )
      return 1
    }
    throw err
  } finally {
    if (agent) {
      await agent[Symbol.asyncDispose]()
    }
  }

  if (!result || result.status !== 'finished' || !agentId || !runId) {
    const reason = result?.error?.message ?? `status=${result?.status ?? 'unknown'}`
    console.error(`[issue-triage] Run did not finish cleanly: ${reason}`)
    return 2
  }

  const outputText = result.result ?? ''
  let analysis: TriageAnalysis
  try {
    analysis = parseTriageAnalysisFromText(outputText)
  } catch (err) {
    console.error(
      `[issue-triage] Failed to parse triage analysis JSON: ${err instanceof Error ? err.message : String(err)}`
    )
    console.error(outputText.slice(0, 2000))
    return 2
  }

  const saved: SavedTriageReport = {
    version: 1,
    startedAt,
    finishedAt: new Date().toISOString(),
    repoSlug: options.repoSlug,
    headSha,
    issueNumber,
    runtime: options.runtime,
    agentId,
    runId,
    analysis
  }
  const reportPath = options.reportOutPath ?? resolve(tmpdir(), `marktext-triage-${issueNumber}-${randomUUID()}.json`)
  writeJson(reportPath, saved)
  console.log(`[issue-triage] Saved triage analysis to ${reportPath}`)

  if (options.reportOnly) return 0
  publishSavedTriage(repoRoot, options, saved)
  return 0
}

const isEntrypoint =
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)

if (isEntrypoint) {
  runIssueTriage()
    .then((code) => process.exit(code))
    .catch((err) => {
      console.error(`[issue-triage] Fatal error: ${err instanceof Error ? err.message : String(err)}`)
      process.exit(1)
    })
}
