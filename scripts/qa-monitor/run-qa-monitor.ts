import { execFileSync } from 'node:child_process'
import { mkdirSync, writeFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import {
  Agent,
  CursorAgentError,
  type AgentOptions,
  type RunResult
} from '@cursor/sdk'
import {
  collectChangeAuditContext,
  resolveGitSha,
  type ChangeAuditContext
} from './change-detector.ts'
import {
  fileAuditFindings,
  listOpenRegressionIssues,
  type FiledIssueRecord
} from './issue-filer.ts'
import { buildQaAuditPrompt } from './prompt.ts'
import {
  parseAuditReportFromText,
  type QaAuditReport,
  type SeverityLevel
} from './rubric.ts'

export type QaRuntime = 'cloud' | 'local'

export interface CliOptions {
  runtime: QaRuntime
  repoSlug: string
  baseSha: string | null
  headSha: string | null
  slice: string
  model: string | null
  dryRun: boolean
  force: boolean
  checkOnly: boolean
  outPath: string | null
}

export interface QaMonitorTelemetry {
  startedAt: string
  finishedAt: string
  durationMs: number
  runtime: QaRuntime
  repoSlug: string
  baseSha: string | null
  headSha: string
  skipped: boolean
  skipReason: string | null
  slicesAudited: string[]
  agentId: string | null
  runId: string | null
  runStatus: RunResult['status'] | null
  findingsTotal: number
  findingsBySeverity: Record<SeverityLevel, number>
  issuesFiled: number
  duplicatesSkipped: number
  dryRun: boolean
  summary: string
  filedIssues: Array<{
    severity: SeverityLevel
    slice: string
    title: string
    fingerprint: string
    status: FiledIssueRecord['status']
    issueNumber?: number
    issueUrl?: string
    duplicateOf?: number
  }>
}

const DEFAULT_REPO_SLUG = 'shashu10/marktext'
const DEFAULT_LOCAL_MODEL = 'composer-2.5'

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

export function parseCliArgs(argv: string[], repoRoot: string): CliOptions {
  let runtime: QaRuntime = 'cloud'
  let repoSlug = inferRepoSlug(repoRoot)
  let baseSha: string | null = null
  let headSha: string | null = null
  let slice = 'auto'
  let model: string | null = process.env.CURSOR_QA_MODEL ?? null
  let dryRun = false
  let force = false
  let checkOnly = false
  let outPath: string | null = null

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
        if (val !== 'cloud' && val !== 'local') {
          throw new Error(`Invalid --runtime "${val}" (expected "cloud" or "local")`)
        }
        runtime = val
        break
      }
      case '--repo':
        repoSlug = next()
        break
      case '--base-sha':
        baseSha = next()
        break
      case '--head-sha':
        headSha = next()
        break
      case '--slice':
        slice = next()
        break
      case '--model':
        model = next()
        break
      case '--out':
        outPath = next()
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
      case '-h': {
        printUsage()
        process.exit(0)
        break
      }
      default:
        throw new Error(`Unknown argument: ${arg}`)
    }
  }

  return {
    runtime,
    repoSlug,
    baseSha,
    headSha,
    slice,
    model,
    dryRun,
    force,
    checkOnly,
    outPath
  }
}

function printUsage(): void {
  console.log(`Usage: node --no-warnings scripts/qa-monitor/run-qa-monitor.ts [options]

Options:
  --runtime <cloud|local>  Cursor SDK runtime (default: cloud)
  --repo <owner/name>      GitHub repository slug (default: inferred from origin)
  --base-sha <sha>         Last audited commit SHA (default: qa/last-audited tag if present)
  --head-sha <sha>         Target commit SHA to audit (default: HEAD)
  --slice <auto|ID>        Slice ID to audit (e.g., A1) or "auto" (default: auto)
  --model <model-id>       Cursor model ID (optional for cloud; default "${DEFAULT_LOCAL_MODEL}" for local)
  --dry-run                Run QA audit and print findings without filing GitHub issues
  --force                  Bypass the no-change skip gate
  --check-only             Evaluate the change gate and print JSON without starting an agent
  --out <file.json>        Write structured QA telemetry JSON to disk
`)
}

export function buildAgentOptions(params: {
  options: CliOptions
  repoRoot: string
  context: ChangeAuditContext
  apiKey: string
}): AgentOptions {
  const { options, repoRoot, context, apiKey } = params
  const sliceNames = context.targetSlices.map((s) => s.id).join(',')
  const shortHead = context.headSha.slice(0, 7)

  const base: AgentOptions = {
    apiKey,
    name: `MarkText QA Monitor (${shortHead}) [${sliceNames}]`
  }

  if (options.runtime === 'cloud') {
    if (options.model) {
      base.model = { id: options.model }
    }
    base.cloud = {
      repos: [
        {
          url: `https://github.com/${options.repoSlug}`,
          startingRef: context.headSha
        }
      ],
      workOnCurrentBranch: false,
      autoCreatePR: false,
      skipReviewerRequest: true,
      metadata: {
        workflow: 'migration-qa-monitor',
        repo: options.repoSlug,
        head_sha: context.headSha,
        base_sha: context.baseSha ?? 'baseline',
        slices: sliceNames
      }
    }
    // Idempotency key avoids duplicate cloud VM spin-ups on retried CI jobs for the same commit range.
    if (!options.force) {
      base.idempotencyKey = `qa-monitor-${context.baseSha?.slice(0, 8) ?? 'init'}-${shortHead}-${sliceNames}`
    }
  } else {
    base.model = { id: options.model ?? DEFAULT_LOCAL_MODEL }
    base.local = { cwd: repoRoot }
    // Hard-enforce non-modifying QA charter on local runtime.
    base.disallowedTools = ['edit', 'delete']
  }

  return base
}

function emptySeverityCounts(): Record<SeverityLevel, number> {
  return { P0: 0, P1: 0, P2: 0, P3: 0 }
}

function writeTelemetry(outPath: string | null, telemetry: QaMonitorTelemetry): void {
  if (!outPath) return
  mkdirSync(dirname(outPath), { recursive: true })
  writeFileSync(outPath, JSON.stringify(telemetry, null, 2) + '\n', 'utf8')
}

async function runQaMonitor(): Promise<number> {
  const startedMs = Date.now()
  const startedAt = new Date(startedMs).toISOString()
  const scriptDir = dirname(fileURLToPath(import.meta.url))
  const repoRoot = resolve(scriptDir, '../..')

  const options = parseCliArgs(process.argv.slice(2), repoRoot)
  const headSha = options.headSha
    ? (resolveGitSha(repoRoot, options.headSha) ?? options.headSha)
    : (resolveGitSha(repoRoot, 'HEAD') ?? '')

  if (!headSha) {
    throw new Error('Could not resolve HEAD commit SHA')
  }

  const baseSha = options.baseSha
    ? (resolveGitSha(repoRoot, options.baseSha) ?? options.baseSha)
    : resolveGitSha(repoRoot, 'refs/tags/qa/last-audited')

  const context = collectChangeAuditContext({
    repoRoot,
    baseSha,
    headSha,
    requestedSlice: options.slice,
    force: options.force
  })

  if (options.checkOnly) {
    const gateResult = {
      shouldRun: context.hasRelevantChanges,
      skipReason: context.skipReason,
      baseSha: context.baseSha,
      headSha: context.headSha,
      commitCount: context.commits.length,
      changedFileCount: context.changedFiles.length,
      targetSlices: context.targetSlices.map((s) => s.id)
    }
    console.log(JSON.stringify(gateResult, null, 2))
    return 0
  }

  if (!context.hasRelevantChanges) {
    console.log(`[qa-monitor] Skipping agent spin-up: ${context.skipReason}`)
    const telemetry: QaMonitorTelemetry = {
      startedAt,
      finishedAt: new Date().toISOString(),
      durationMs: Date.now() - startedMs,
      runtime: options.runtime,
      repoSlug: options.repoSlug,
      baseSha: context.baseSha,
      headSha: context.headSha,
      skipped: true,
      skipReason: context.skipReason,
      slicesAudited: [],
      agentId: null,
      runId: null,
      runStatus: null,
      findingsTotal: 0,
      findingsBySeverity: emptySeverityCounts(),
      issuesFiled: 0,
      duplicatesSkipped: 0,
      dryRun: options.dryRun,
      summary: context.skipReason ?? 'Skipped — no relevant changes.',
      filedIssues: []
    }
    writeTelemetry(options.outPath, telemetry)
    return 0
  }

  const apiKey = process.env.CURSOR_API_KEY
  if (!apiKey) {
    console.error('[qa-monitor] Error: CURSOR_API_KEY environment variable is required.')
    return 1
  }

  const openIssues = listOpenRegressionIssues(repoRoot, options.repoSlug)
  const prompt = buildQaAuditPrompt({
    repoSlug: options.repoSlug,
    context,
    openIssues
  })

  console.log(
    `[qa-monitor] Starting ${options.runtime} QA audit on ${options.repoSlug} ` +
    `(${context.baseSha ? `${context.baseSha.slice(0, 8)}..` : ''}${context.headSha.slice(0, 8)}) ` +
    `for slice(s): ${context.targetSlices.map((s) => s.id).join(', ')}`
  )

  const agentOptions = buildAgentOptions({ options, repoRoot, context, apiKey })
  let agent: Awaited<ReturnType<typeof Agent.create>> | null = null
  let runId: string | null
  let agentId: string | null
  let result: RunResult | null

  try {
    agent = await Agent.create(agentOptions)
    agentId = agent.agentId
    console.log(`[qa-monitor] Agent created: agentId=${agentId} (runtime=${options.runtime})`)

    const run = await agent.send(prompt)
    runId = run.id
    console.log(`[qa-monitor] Run started: runId=${runId}`)

    for await (const event of run.stream()) {
      if (event.type === 'status') {
        const msg = event.message ? ` (${event.message})` : ''
        console.log(`[qa-monitor] status: ${event.status}${msg}`)
      } else if (event.type === 'tool_call' && event.status === 'running') {
        console.log(`[qa-monitor] tool: ${event.name}`)
      } else if (event.type === 'summary') {
        console.log(`[qa-monitor] compaction summary: ${event.status}`)
      }
    }

    result = await run.wait()
  } catch (err) {
    if (err instanceof CursorAgentError) {
      console.error(
        `[qa-monitor] CursorAgentError (code=${err.code ?? 'unknown'}, retryable=${String(err.isRetryable)}): ${err.message}`
      )
      return 1
    }
    throw err
  } finally {
    if (agent) {
      await agent[Symbol.asyncDispose]()
    }
  }

  if (!result || result.status !== 'finished') {
    const reason = result?.error?.message ?? `status=${result?.status ?? 'unknown'}`
    console.error(`[qa-monitor] QA run did not finish cleanly: ${reason}`)
    return 2
  }

  const outputText = result.result ?? ''
  let report: QaAuditReport
  try {
    report = parseAuditReportFromText(outputText)
  } catch (err) {
    console.error(
      `[qa-monitor] Failed to parse structured QA findings JSON: ${err instanceof Error ? err.message : String(err)}`
    )
    console.error('[qa-monitor] Raw agent final output (truncated to 2000 chars):')
    console.error(outputText.slice(0, 2000))
    return 2
  }

  const filedRecords = fileAuditFindings({
    repoRoot,
    report,
    existingIssues: openIssues,
    meta: {
      repoSlug: options.repoSlug,
      baseSha: context.baseSha,
      headSha: context.headSha,
      agentId,
      runId,
      runtime: options.runtime,
      dryRun: options.dryRun
    }
  })

  const severityCounts = emptySeverityCounts()
  for (const finding of report.findings) {
    severityCounts[finding.severity]++
  }

  const issuesFiled = filedRecords.filter((r) => r.status === 'filed').length
  const duplicatesSkipped = filedRecords.filter((r) => r.status === 'skipped-duplicate').length

  const telemetry: QaMonitorTelemetry = {
    startedAt,
    finishedAt: new Date().toISOString(),
    durationMs: Date.now() - startedMs,
    runtime: options.runtime,
    repoSlug: options.repoSlug,
    baseSha: context.baseSha,
    headSha: context.headSha,
    skipped: false,
    skipReason: null,
    slicesAudited: report.slicesAudited.length > 0
      ? report.slicesAudited
      : context.targetSlices.map((s) => s.id),
    agentId,
    runId,
    runStatus: result.status,
    findingsTotal: report.findings.length,
    findingsBySeverity: severityCounts,
    issuesFiled,
    duplicatesSkipped,
    dryRun: options.dryRun,
    summary: report.summary,
    filedIssues: filedRecords.map((rec) => ({
      severity: rec.finding.severity,
      slice: rec.finding.slice,
      title: rec.finding.title,
      fingerprint: rec.finding.fingerprint,
      status: rec.status,
      issueNumber: rec.issueNumber,
      issueUrl: rec.issueUrl,
      duplicateOf: rec.duplicateOf
    }))
  }

  writeTelemetry(options.outPath, telemetry)

  console.log('\n[qa-monitor] === Audit Complete ===')
  console.log(`[qa-monitor] Summary: ${report.summary}`)
  console.log(
    `[qa-monitor] Findings: ${report.findings.length} ` +
    `(P0=${severityCounts.P0}, P1=${severityCounts.P1}, P2=${severityCounts.P2}, P3=${severityCounts.P3}) | ` +
    `Filed: ${issuesFiled} | Duplicates skipped: ${duplicatesSkipped} | Dry run: ${options.dryRun}`
  )
  for (const rec of filedRecords) {
    const suffix = rec.issueUrl
      ? ` -> ${rec.issueUrl}`
      : rec.duplicateOf
        ? ` (duplicate of #${rec.duplicateOf})`
        : ''
    console.log(
      `  - [${rec.finding.severity}][${rec.finding.slice}] (${rec.status}) ${rec.finding.title}${suffix}`
    )
  }

  return 0
}

const isEntrypoint =
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)

if (isEntrypoint) {
  runQaMonitor()
    .then((code) => process.exit(code))
    .catch((err) => {
      console.error(`[qa-monitor] Fatal error: ${err instanceof Error ? err.message : String(err)}`)
      process.exit(1)
    })
}
