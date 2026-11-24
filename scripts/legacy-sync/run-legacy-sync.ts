import { execFileSync } from 'node:child_process'
import { randomUUID } from 'node:crypto'
import { mkdirSync, renameSync, writeFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import type { AgentOptions, RunResult } from '@cursor/sdk'
import { resolveGitSha } from '../qa-monitor/change-detector.ts'
import { collectDriftContext, type DriftedSlice } from './drift-detector.ts'
import { buildLegacySyncPrompt } from './prompt.ts'

export type SyncRuntime = 'cloud' | 'local'

export interface LegacySyncCliOptions {
  runtime: SyncRuntime
  repoSlug: string
  baseSha: string | null
  headSha: string | null
  slice: string
  model: string | null
  noPr: boolean
  dryRun: boolean
  force: boolean
  checkOnly: boolean
  outPath: string | null
}

export interface LegacySyncRunRecord {
  sliceId: string
  rustCrate: string
  changedLegacyFiles: string[]
  agentId: string | null
  runId: string | null
  runStatus: RunResult['status'] | 'dry-run'
  summary: string
}

export interface LegacySyncTelemetry {
  startedAt: string
  finishedAt: string
  durationMs: number
  runtime: SyncRuntime
  repoSlug: string
  baseSha: string
  headSha: string
  skipped: boolean
  skipReason: string | null
  autoCreatePR: boolean
  dryRun: boolean
  runs: LegacySyncRunRecord[]
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

export function parseLegacySyncCliArgs(argv: string[], repoRoot: string): LegacySyncCliOptions {
  let runtime: SyncRuntime = 'cloud'
  let repoSlug = inferRepoSlug(repoRoot)
  let baseSha: string | null = null
  let headSha: string | null = null
  let slice = 'auto'
  let model: string | null = process.env.CURSOR_SYNC_MODEL ?? null
  let noPr = false
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
      case '--no-pr':
        noPr = true
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
      case '--out':
        outPath = next()
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

  return {
    runtime,
    repoSlug,
    baseSha,
    headSha,
    slice,
    model,
    noPr,
    dryRun,
    force,
    checkOnly,
    outPath
  }
}

function printUsage(): void {
  console.log(`Usage: node --no-warnings scripts/legacy-sync/run-legacy-sync.ts [options]

Options:
  --runtime <cloud|local>  Cursor SDK runtime (default: cloud — compiles Rust in pre-warmed VM)
  --repo <owner/name>      GitHub repository slug (default: inferred from origin)
  --base-sha <sha>         Base commit SHA for diff (default: HEAD~1)
  --head-sha <sha>         Head commit SHA for diff (default: HEAD)
  --slice <auto|ID>        Target migrated slice ID (e.g., A1) or "auto" (default: auto)
  --model <model-id>       Cursor model ID (default: "${DEFAULT_MODEL}")
  --no-pr                  Disable autoCreatePR on cloud runtime (analysis/branch only)
  --check-only             Evaluate deterministic parity drift gate and print JSON ($0)
  --dry-run                Detect drift and build prompts without starting a Cursor agent
  --force                  Run even if the Rust crate was also touched in the same range
  --out <file.json>        Write structured telemetry JSON to disk
`)
}

export function buildSyncAgentOptions(params: {
  options: LegacySyncCliOptions
  repoRoot: string
  baseSha: string
  headSha: string
  drifted: DriftedSlice
  apiKey: string
}): AgentOptions {
  const { options, repoRoot, baseSha, headSha, drifted, apiKey } = params
  const shortBase = baseSha.slice(0, 7)
  const shortHead = headSha.slice(0, 7)
  const modelId = options.model ?? DEFAULT_MODEL

  const base: AgentOptions = {
    apiKey,
    name: `MarkText Legacy Sync ${drifted.slice.id} (${shortHead})`,
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
      autoCreatePR: !options.noPr,
      skipReviewerRequest: true,
      metadata: {
        workflow: 'legacy-to-rust-sync',
        repo: options.repoSlug,
        slice: drifted.slice.id,
        rust_crate: drifted.slice.rustCrate,
        base_sha: baseSha,
        head_sha: headSha
      }
    }
    if (!options.force) {
      base.idempotencyKey = `legacy-sync-${shortBase}-${shortHead}-${drifted.slice.id}`
    }
  } else {
    base.local = { cwd: repoRoot }
  }

  return base
}

function writeJson(outPath: string, data: LegacySyncTelemetry): void {
  mkdirSync(dirname(outPath), { recursive: true })
  const temporary = `${outPath}.${randomUUID()}.tmp`
  writeFileSync(temporary, JSON.stringify(data, null, 2) + '\n', { mode: 0o600 })
  renameSync(temporary, outPath)
}

async function runLegacySync(): Promise<number> {
  const startedMs = Date.now()
  const startedAt = new Date(startedMs).toISOString()
  const scriptDir = dirname(fileURLToPath(import.meta.url))
  const repoRoot = resolve(scriptDir, '../..')
  const options = parseLegacySyncCliArgs(process.argv.slice(2), repoRoot)

  const headSha = options.headSha
    ? (resolveGitSha(repoRoot, options.headSha) ?? options.headSha)
    : (resolveGitSha(repoRoot, 'HEAD') ?? '')
  if (!headSha) {
    throw new Error('Could not resolve head commit SHA')
  }

  const baseSha = options.baseSha
    ? (resolveGitSha(repoRoot, options.baseSha) ?? options.baseSha)
    : (resolveGitSha(repoRoot, `${headSha}~1`) ?? '')
  if (!baseSha) {
    throw new Error('Could not resolve base commit SHA (pass --base-sha explicitly)')
  }

  const context = collectDriftContext({
    repoRoot,
    baseSha,
    headSha,
    requestedSlice: options.slice,
    force: options.force
  })

  if (options.checkOnly) {
    console.log(JSON.stringify({
      shouldRun: context.driftedSlices.length > 0,
      skipReason: context.skipReason,
      baseSha: context.baseSha,
      headSha: context.headSha,
      commitCount: context.commits.length,
      changedFileCount: context.changedFiles.length,
      driftedSlices: context.driftedSlices.map((item) => ({
        id: item.slice.id,
        rustCrate: item.slice.rustCrate,
        changedLegacyFiles: item.changedLegacyFiles
      }))
    }, null, 2))
    return 0
  }

  if (context.driftedSlices.length === 0) {
    console.log(`[legacy-sync] Skipping agent spin-up: ${context.skipReason}`)
    const telemetry: LegacySyncTelemetry = {
      startedAt,
      finishedAt: new Date().toISOString(),
      durationMs: Date.now() - startedMs,
      runtime: options.runtime,
      repoSlug: options.repoSlug,
      baseSha,
      headSha,
      skipped: true,
      skipReason: context.skipReason,
      autoCreatePR: options.runtime === 'cloud' && !options.noPr,
      dryRun: options.dryRun,
      runs: []
    }
    if (options.outPath) writeJson(options.outPath, telemetry)
    return 0
  }

  if (options.dryRun) {
    const runs: LegacySyncRunRecord[] = context.driftedSlices.map((drifted) => {
      const prompt = buildLegacySyncPrompt({
        repoSlug: options.repoSlug,
        baseSha,
        headSha,
        drifted,
        commits: context.commits
      })
      console.log(
        `[legacy-sync] [dry-run] Would sync slice ${drifted.slice.id} (${drifted.slice.rustCrate}) ` +
        `from ${drifted.changedLegacyFiles.length} legacy file(s) (prompt ${prompt.length} chars).`
      )
      return {
        sliceId: drifted.slice.id,
        rustCrate: drifted.slice.rustCrate,
        changedLegacyFiles: drifted.changedLegacyFiles,
        agentId: null,
        runId: null,
        runStatus: 'dry-run',
        summary: `Dry-run prompt prepared (${prompt.length} chars).`
      }
    })
    const telemetry: LegacySyncTelemetry = {
      startedAt,
      finishedAt: new Date().toISOString(),
      durationMs: Date.now() - startedMs,
      runtime: options.runtime,
      repoSlug: options.repoSlug,
      baseSha,
      headSha,
      skipped: false,
      skipReason: null,
      autoCreatePR: options.runtime === 'cloud' && !options.noPr,
      dryRun: true,
      runs
    }
    if (options.outPath) writeJson(options.outPath, telemetry)
    return 0
  }

  const apiKey = process.env.CURSOR_API_KEY
  if (!apiKey) {
    console.error('[legacy-sync] Error: CURSOR_API_KEY environment variable is required.')
    return 1
  }

  const { Agent, CursorAgentError } = await import('@cursor/sdk')
  const runs: LegacySyncRunRecord[] = []
  let exitCode = 0

  for (const drifted of context.driftedSlices) {
    const prompt = buildLegacySyncPrompt({
      repoSlug: options.repoSlug,
      baseSha,
      headSha,
      drifted,
      commits: context.commits
    })
    const agentOptions = buildSyncAgentOptions({
      options,
      repoRoot,
      baseSha,
      headSha,
      drifted,
      apiKey
    })

    console.log(
      `[legacy-sync] Starting ${options.runtime} forward-port for slice ${drifted.slice.id} ` +
      `(${drifted.slice.rustCrate}) on ${options.repoSlug} (${baseSha.slice(0, 8)}..${headSha.slice(0, 8)})`
    )

    let agent: Awaited<ReturnType<typeof Agent.create>> | null = null
    let agentId: string | null
    let runId: string | null
    let result: RunResult | null

    try {
      agent = await Agent.create(agentOptions)
      agentId = agent.agentId
      console.log(`[legacy-sync] Agent created: agentId=${agentId} (slice=${drifted.slice.id})`)

      const run = await agent.send(prompt)
      runId = run.id
      console.log(`[legacy-sync] Run started: runId=${runId} (slice=${drifted.slice.id})`)

      for await (const event of run.stream()) {
        if (event.type === 'status') {
          const msg = event.message ? ` (${event.message})` : ''
          console.log(`[legacy-sync][${drifted.slice.id}] status: ${event.status}${msg}`)
        } else if (event.type === 'tool_call' && event.status === 'running') {
          console.log(`[legacy-sync][${drifted.slice.id}] tool: ${event.name}`)
        } else if (event.type === 'summary') {
          console.log(`[legacy-sync][${drifted.slice.id}] summary: ${event.status}`)
        }
      }

      result = await run.wait()
    } catch (err) {
      if (err instanceof CursorAgentError) {
        console.error(
          `[legacy-sync] CursorAgentError for ${drifted.slice.id} ` +
          `(code=${err.code ?? 'unknown'}, retryable=${String(err.isRetryable)}): ${err.message}`
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
      console.error(`[legacy-sync] Slice ${drifted.slice.id} did not finish cleanly: ${reason}`)
      exitCode = 2
      runs.push({
        sliceId: drifted.slice.id,
        rustCrate: drifted.slice.rustCrate,
        changedLegacyFiles: drifted.changedLegacyFiles,
        agentId,
        runId,
        runStatus: result?.status ?? 'error',
        summary: reason
      })
      continue
    }

    runs.push({
      sliceId: drifted.slice.id,
      rustCrate: drifted.slice.rustCrate,
      changedLegacyFiles: drifted.changedLegacyFiles,
      agentId,
      runId,
      runStatus: result.status,
      summary: (result.result ?? '').slice(0, 500)
    })
  }

  const telemetry: LegacySyncTelemetry = {
    startedAt,
    finishedAt: new Date().toISOString(),
    durationMs: Date.now() - startedMs,
    runtime: options.runtime,
    repoSlug: options.repoSlug,
    baseSha,
    headSha,
    skipped: false,
    skipReason: null,
    autoCreatePR: options.runtime === 'cloud' && !options.noPr,
    dryRun: false,
    runs
  }
  if (options.outPath) writeJson(options.outPath, telemetry)

  return exitCode
}

const isEntrypoint =
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)

if (isEntrypoint) {
  runLegacySync()
    .then((code) => process.exit(code))
    .catch((err) => {
      console.error(`[legacy-sync] Fatal error: ${err instanceof Error ? err.message : String(err)}`)
      process.exit(1)
    })
}
