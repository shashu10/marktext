import { execFileSync } from 'node:child_process'
import { existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'

export interface SliceMetadata {
  id: string
  name: string
  surface: string
  flag: string
  rustCrate: string
  legacyPaths: string[]
}

export interface CommitSummary {
  sha: string
  shortSha: string
  subject: string
  author: string
  date: string
  prNumber: number | null
}

export interface ChangeAuditContext {
  baseSha: string | null
  headSha: string
  hasRelevantChanges: boolean
  skipReason: string | null
  commits: CommitSummary[]
  changedFiles: string[]
  targetSlices: SliceMetadata[]
  allActiveSlices: SliceMetadata[]
}

const RELEVANT_PATH_PREFIXES = [
  'rust/',
  'packages/desktop/src/',
  'packages/muya/src/',
  'scripts/migration-measure.ts',
  'docs/migration/STATUS.md',
  'docs/migration/slices.json'
]

export function isRelevantMigrationPath(filePath: string): boolean {
  const normalized = filePath.replace(/\\/g, '/')
  return RELEVANT_PATH_PREFIXES.some(
    (prefix) => normalized === prefix || normalized.startsWith(prefix)
  )
}

export function extractPrNumberFromSubject(subject: string): number | null {
  const mergeMatch = subject.match(/Merge pull request #(\d+)\b/i)
  if (mergeMatch?.[1]) return Number(mergeMatch[1])
  const squashMatch = subject.match(/\(#(\d+)\)\s*$/)
  if (squashMatch?.[1]) return Number(squashMatch[1])
  return null
}

export function parseStatusActiveSliceIds(statusMarkdown: string): Set<string> {
  const active = new Set<string>()
  for (const line of statusMarkdown.split('\n')) {
    if (!line.trim().startsWith('|')) continue
    const cells = line.split('|').map((c) => c.trim()).filter(Boolean)
    if (cells.length < 2) continue
    const rawId = cells[0]?.replace(/`/g, '') ?? ''
    const rawStatus = cells[cells.length - 1]?.replace(/`/g, '').toLowerCase() ?? ''
    if (!rawId || rawId === 'Slice ID' || rawId.startsWith('---')) continue
    if (rawStatus === 'in-review' || rawStatus === 'verified' || rawStatus === 'in-progress') {
      if (!rawId.startsWith('P0.')) {
        active.add(rawId)
      }
    }
  }
  return active
}

export function loadSlicesCatalog(repoRoot: string): {
  slices: SliceMetadata[]
  activeSlices: SliceMetadata[]
} {
  const catalogPath = resolve(repoRoot, 'docs/migration/slices.json')
  const statusPath = resolve(repoRoot, 'docs/migration/STATUS.md')

  const parsed = JSON.parse(readFileSync(catalogPath, 'utf8')) as { slices: SliceMetadata[] }
  const slices = parsed.slices

  const activeIds = existsSync(statusPath)
    ? parseStatusActiveSliceIds(readFileSync(statusPath, 'utf8'))
    : new Set<string>()

  const activeSlices = existsSync(statusPath)
    ? slices.filter((slice) => activeIds.has(slice.id))
    : slices.filter((slice) => existsSync(resolve(repoRoot, slice.rustCrate)))

  return { slices, activeSlices }
}

export function matchSlicesForFiles(
  changedFiles: string[],
  slices: SliceMetadata[],
  activeSlices: SliceMetadata[]
): SliceMetadata[] {
  const matched = new Map<string, SliceMetadata>()
  const sharedHostTouched = changedFiles.some(
    (file) =>
      file.startsWith('rust/crates/marktext-core/') ||
      file.startsWith('rust/crates/marktext-ui-kit/') ||
      file.startsWith('rust/crates/marktext-wasm-host/') ||
      file.startsWith('packages/desktop/src/renderer/src/components/gpuiHost/') ||
      file.startsWith('packages/desktop/src/renderer/public/gpui-host/') ||
      file.startsWith('packages/desktop/src/shared/flags/') ||
      file.startsWith('packages/desktop/src/renderer/src/prefComponents/preferenceRoute.vue')
  )

  if (sharedHostTouched) {
    for (const slice of activeSlices) {
      matched.set(slice.id, slice)
    }
  }

  for (const file of changedFiles) {
    const normalized = file.replace(/\\/g, '/')
    for (const slice of slices) {
      const hitsCrate =
        normalized === slice.rustCrate || normalized.startsWith(`${slice.rustCrate}/`)
      const hitsLegacy = slice.legacyPaths.some(
        (legacy) => normalized === legacy || normalized.startsWith(`${legacy}/`)
      )
      if (hitsCrate || hitsLegacy) {
        // Only audit slices that actually have a Rust implementation (or are active).
        if (activeSlices.some((active) => active.id === slice.id)) {
          matched.set(slice.id, slice)
        }
      }
    }
  }

  return [...matched.values()]
}

function runGit(repoRoot: string, args: string[]): string {
  return execFileSync('git', args, {
    cwd: repoRoot,
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe']
  }).trim()
}

export function resolveGitSha(repoRoot: string, ref: string): string | null {
  try {
    return runGit(repoRoot, ['rev-parse', '--verify', `${ref}^{commit}`])
  } catch {
    return null
  }
}

export function collectChangeAuditContext(options: {
  repoRoot: string
  baseSha: string | null
  headSha: string
  requestedSlice: string | null
  force: boolean
}): ChangeAuditContext {
  const { repoRoot, baseSha, headSha, requestedSlice, force } = options
  const { slices, activeSlices } = loadSlicesCatalog(repoRoot)

  if (requestedSlice && requestedSlice !== 'auto') {
    const chosen = slices.find((s) => s.id.toLowerCase() === requestedSlice.toLowerCase())
    if (!chosen) {
      throw new Error(`Unknown slice "${requestedSlice}" in docs/migration/slices.json`)
    }
    const commits = readCommitRange(repoRoot, baseSha, headSha)
    const changedFiles = readChangedFiles(repoRoot, baseSha, headSha)
    return {
      baseSha,
      headSha,
      hasRelevantChanges: true,
      skipReason: null,
      commits,
      changedFiles,
      targetSlices: [chosen],
      allActiveSlices: activeSlices
    }
  }

  if (baseSha && baseSha === headSha && !force) {
    return {
      baseSha,
      headSha,
      hasRelevantChanges: false,
      skipReason: `HEAD (${headSha.slice(0, 8)}) matches last audited commit — no new changes.`,
      commits: [],
      changedFiles: [],
      targetSlices: [],
      allActiveSlices: activeSlices
    }
  }

  const commits = readCommitRange(repoRoot, baseSha, headSha)
  const changedFiles = readChangedFiles(repoRoot, baseSha, headSha)
  const relevantFiles = changedFiles.filter(isRelevantMigrationPath)

  if (!force && baseSha && relevantFiles.length === 0) {
    return {
      baseSha,
      headSha,
      hasRelevantChanges: false,
      skipReason: `Found ${commits.length} commit(s), but none modified app/migration paths.`,
      commits,
      changedFiles,
      targetSlices: [],
      allActiveSlices: activeSlices
    }
  }

  const matchedSlices = matchSlicesForFiles(changedFiles, slices, activeSlices)
  const targetSlices = matchedSlices.length > 0 ? matchedSlices : activeSlices

  if (targetSlices.length === 0) {
    return {
      baseSha,
      headSha,
      hasRelevantChanges: false,
      skipReason: 'No active migrated Rust/GPUI slices found to audit yet.',
      commits,
      changedFiles,
      targetSlices: [],
      allActiveSlices: activeSlices
    }
  }

  return {
    baseSha,
    headSha,
    hasRelevantChanges: true,
    skipReason: null,
    commits,
    changedFiles,
    targetSlices,
    allActiveSlices: activeSlices
  }
}

function readCommitRange(
  repoRoot: string,
  baseSha: string | null,
  headSha: string
): CommitSummary[] {
  const rangeArgs = baseSha
    ? [`${baseSha}..${headSha}`]
    : ['-n', '10', headSha]

  let output: string
  try {
    output = runGit(repoRoot, [
      'log',
      '--no-merges',
      '--format=%H%x1f%h%x1f%s%x1f%an%x1f%aI',
      ...rangeArgs
    ])
  } catch {
    return []
  }

  if (!output) return []
  return output.split('\n').filter(Boolean).map((line) => {
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
}

function readChangedFiles(
  repoRoot: string,
  baseSha: string | null,
  headSha: string
): string[] {
  try {
    const args = baseSha
      ? ['diff', '--name-only', `${baseSha}..${headSha}`]
      : ['diff-tree', '--no-commit-id', '--name-only', '-r', headSha]
    const output = runGit(repoRoot, args)
    return output ? output.split('\n').map((l) => l.trim()).filter(Boolean) : []
  } catch {
    return []
  }
}
