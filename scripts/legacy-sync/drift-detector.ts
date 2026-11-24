import { execFileSync } from 'node:child_process'
import {
  extractPrNumberFromSubject,
  loadSlicesCatalog,
  type CommitSummary,
  type SliceMetadata
} from '../qa-monitor/change-detector.ts'

export interface DriftedSlice {
  slice: SliceMetadata
  changedLegacyFiles: string[]
  changedRustFiles: string[]
  legacyDiff: string
}

export interface DriftDetectionContext {
  baseSha: string
  headSha: string
  commits: CommitSummary[]
  changedFiles: string[]
  driftedSlices: DriftedSlice[]
  skipReason: string | null
}

function normalizePath(filePath: string): string {
  return filePath.replace(/\\/g, '/').replace(/^\.\//, '')
}

function matchesPrefixOrExact(filePath: string, pattern: string): boolean {
  const file = normalizePath(filePath)
  const target = normalizePath(pattern).replace(/\/$/, '')
  return file === target || file.startsWith(`${target}/`)
}

export function findDriftedSlices(params: {
  changedFiles: string[]
  activeSlices: SliceMetadata[]
  requestedSlice?: string | null
  force?: boolean
}): Array<{ slice: SliceMetadata; changedLegacyFiles: string[]; changedRustFiles: string[] }> {
  const { changedFiles, activeSlices, requestedSlice, force = false } = params
  const normalizedFiles = changedFiles.map(normalizePath)

  const candidates = requestedSlice && requestedSlice !== 'auto'
    ? activeSlices.filter((s) => s.id.toLowerCase() === requestedSlice.toLowerCase())
    : activeSlices

  const results: Array<{
    slice: SliceMetadata
    changedLegacyFiles: string[]
    changedRustFiles: string[]
  }> = []

  for (const slice of candidates) {
    const changedLegacyFiles = normalizedFiles.filter((file) =>
      slice.legacyPaths.some((legacyPath) => matchesPrefixOrExact(file, legacyPath))
    )
    const changedRustFiles = normalizedFiles.filter((file) =>
      matchesPrefixOrExact(file, slice.rustCrate)
    )

    if (changedLegacyFiles.length > 0 && (force || changedRustFiles.length === 0)) {
      results.push({ slice, changedLegacyFiles, changedRustFiles })
    } else if (force && requestedSlice && requestedSlice !== 'auto') {
      results.push({ slice, changedLegacyFiles, changedRustFiles })
    }
  }

  return results
}

function runGit(repoRoot: string, args: string[]): string {
  return execFileSync('git', args, {
    cwd: repoRoot,
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
    maxBuffer: 32 * 1024 * 1024
  }).trim()
}

function readCommitsBetween(repoRoot: string, baseSha: string, headSha: string): CommitSummary[] {
  try {
    const raw = runGit(repoRoot, [
      'log',
      '--no-merges',
      '--format=%H%x1f%h%x1f%s%x1f%an%x1f%aI',
      `${baseSha}..${headSha}`
    ])
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

function readChangedFilesBetween(repoRoot: string, baseSha: string, headSha: string): string[] {
  try {
    const raw = runGit(repoRoot, ['diff', '--name-only', `${baseSha}..${headSha}`])
    return raw ? raw.split('\n').map((l) => l.trim()).filter(Boolean) : []
  } catch {
    return []
  }
}

function readBoundedDiff(
  repoRoot: string,
  baseSha: string,
  headSha: string,
  files: string[],
  maxChars = 16000
): string {
  if (files.length === 0) return ''
  try {
    const diff = runGit(repoRoot, ['diff', '--unified=3', `${baseSha}..${headSha}`, '--', ...files])
    if (diff.length <= maxChars) return diff
    return `${diff.slice(0, maxChars)}\n\n...[diff truncated at ${maxChars} chars; inspect full diff with git diff ${baseSha.slice(0, 8)}..${headSha.slice(0, 8)}]`
  } catch {
    return ''
  }
}

export function collectDriftContext(params: {
  repoRoot: string
  baseSha: string
  headSha: string
  requestedSlice: string | null
  force: boolean
}): DriftDetectionContext {
  const { repoRoot, baseSha, headSha, requestedSlice, force } = params
  const { slices, activeSlices } = loadSlicesCatalog(repoRoot)

  if (requestedSlice && requestedSlice !== 'auto') {
    const known = slices.find((s) => s.id.toLowerCase() === requestedSlice.toLowerCase())
    if (!known) {
      throw new Error(`Unknown slice "${requestedSlice}" in docs/migration/slices.json`)
    }
    const isActive = activeSlices.some((s) => s.id === known.id)
    if (!isActive) {
      return {
        baseSha,
        headSha,
        commits: [],
        changedFiles: [],
        driftedSlices: [],
        skipReason: `Slice "${known.id}" is not yet migrated/active in docs/migration/STATUS.md.`
      }
    }
  }

  const commits = readCommitsBetween(repoRoot, baseSha, headSha)
  const changedFiles = readChangedFilesBetween(repoRoot, baseSha, headSha)
  const matched = findDriftedSlices({
    changedFiles,
    activeSlices,
    requestedSlice,
    force
  })

  if (matched.length === 0) {
    return {
      baseSha,
      headSha,
      commits,
      changedFiles,
      driftedSlices: [],
      skipReason: `No migrated slice had legacy-only drift in ${baseSha.slice(0, 8)}..${headSha.slice(0, 8)}.`
    }
  }

  const driftedSlices: DriftedSlice[] = matched.map((item) => ({
    ...item,
    legacyDiff: readBoundedDiff(repoRoot, baseSha, headSha, item.changedLegacyFiles)
  }))

  return {
    baseSha,
    headSha,
    commits,
    changedFiles,
    driftedSlices,
    skipReason: null
  }
}
