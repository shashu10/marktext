#!/usr/bin/env node
import fs from 'node:fs'
import path from 'node:path'

interface SurfaceDef {
  label: string
  paths: string[]
}

interface SliceDef {
  id: string
  name: string
  surface: string
  flag: string
  rustCrate: string
  legacyPaths: string[]
}

interface SliceManifest {
  legacyRoots: string[]
  rustRoots: string[]
  legacyExtensions: string[]
  rustExtensions: string[]
  surfaces: Record<string, SurfaceDef>
  slices: SliceDef[]
}

interface SurfaceStats {
  id: string
  label: string
  totalLoc: number
  beforeSupersededLoc: number
  afterSupersededLoc: number
  beforePct: number
  afterPct: number
  deltaPct: number
}

interface ProgressReport {
  baselineSlices: string[]
  addedSlices: string[]
  totalLegacyLoc: number
  beforeSupersededLoc: number
  afterSupersededLoc: number
  beforePct: number
  afterPct: number
  deltaLoc: number
  deltaPct: number
  remainingLegacyLoc: number
  rustLoc: number
  rustFiles: number
  rustShareOfServedPct: number
  surfaces: SurfaceStats[]
}

const repoRoot = path.resolve(import.meta.dirname, '..')
const manifestPath = path.join(repoRoot, 'docs/migration/slices.json')
const statusPath = path.join(repoRoot, 'docs/migration/STATUS.md')

const isTestPath = (relPath: string): boolean =>
  relPath.includes('/__tests__/') ||
  relPath.includes('/test/') ||
  /\.(spec|test)\.[a-z]+$/i.test(relPath)

const collectFiles = (roots: string[], extensions: Set<string>): Set<string> => {
  const result = new Set<string>()

  const walk = (absPath: string): void => {
    if (!fs.existsSync(absPath)) return
    const stat = fs.statSync(absPath)
    if (stat.isFile()) {
      const rel = path.relative(repoRoot, absPath).split(path.sep).join('/')
      if (!isTestPath(rel) && extensions.has(path.extname(rel))) {
        result.add(rel)
      }
      return
    }
    if (!stat.isDirectory()) return
    for (const entry of fs.readdirSync(absPath, { withFileTypes: true })) {
      if (entry.name === 'node_modules' || entry.name === 'target' || entry.name === 'dist' || entry.name === 'out') {
        continue
      }
      walk(path.join(absPath, entry.name))
    }
  }

  for (const root of roots) {
    walk(path.join(repoRoot, root))
  }
  return result
}

const countLines = (relPath: string): number => {
  const content = fs.readFileSync(path.join(repoRoot, relPath), 'utf8')
  if (content.length === 0) return 0
  let lines = 0
  for (let i = 0; i < content.length; i++) {
    if (content.charCodeAt(i) === 10) lines++
  }
  if (content.charCodeAt(content.length - 1) !== 10) lines++
  return lines
}

const parseStatusFile = (filePath: string, includeInReview: boolean): string[] => {
  if (!fs.existsSync(filePath)) return []
  const raw = fs.readFileSync(filePath, 'utf8')
  const accepted = new Set(includeInReview ? ['verified', 'in-review'] : ['verified'])
  const matched: string[] = []
  for (const line of raw.split('\n')) {
    const m = line.match(/^\|\s*`([^`]+)`\s*\|.*\|\s*`([^`]+)`\s*\|\s*$/)
    if (!m) continue
    const [, sliceId, status] = m
    if (sliceId && status && accepted.has(status.trim())) {
      matched.push(sliceId.trim())
    }
  }
  return matched
}

const parseFlagEnv = (envName: string, manifest: SliceManifest): string[] => {
  const flagFile = path.join(repoRoot, 'flags/environments.json')
  if (!fs.existsSync(flagFile)) return []
  const parsed = JSON.parse(fs.readFileSync(flagFile, 'utf8')) as Record<string, Record<string, string>>
  const envFlags = parsed[envName]
  if (!envFlags) return []
  const rustModes = new Set(['rust-wasm', 'gpui-native', 'rust'])
  return manifest.slices
    .filter((s) => rustModes.has(envFlags[s.flag] ?? ''))
    .map((s) => s.id)
}

const parseArgs = (argv: string[]) => {
  let format: 'markdown' | 'json' = 'markdown'
  let includeInReview = false
  let envName: string | null = null
  const extraSlices: string[] = []

  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i]
    if (arg === '--format' && argv[i + 1]) {
      const val = argv[++i]
      if (val === 'json' || val === 'markdown') format = val
    } else if (arg === '--include-in-review') {
      includeInReview = true
    } else if (arg === '--env' && argv[i + 1]) {
      envName = argv[++i]
    } else if (arg === '--slice' && argv[i + 1]) {
      for (const item of argv[++i].split(',')) {
        const trimmed = item.trim()
        if (trimmed) extraSlices.push(trimmed)
      }
    }
  }
  return { format, includeInReview, envName, extraSlices }
}

const pct = (part: number, total: number): number =>
  total === 0 ? 0 : Math.round((part / total) * 10000) / 100

const progressBar = (percent: number, width = 20): string => {
  const clamped = Math.max(0, Math.min(100, percent))
  const filled = Math.round((clamped / 100) * width)
  return `[${'#'.repeat(filled)}${'-'.repeat(width - filled)}] ${clamped.toFixed(2)}%`
}

const formatDelta = (delta: number, suffix = ''): string => {
  const sign = delta > 0 ? '+' : ''
  return `${sign}${delta.toFixed(2)}${suffix}`
}

const main = (): void => {
  const { format, includeInReview, envName, extraSlices } = parseArgs(process.argv.slice(2))
  const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8')) as SliceManifest
  const legacyExts = new Set(manifest.legacyExtensions)
  const rustExts = new Set(manifest.rustExtensions)

  const allLegacyFiles = collectFiles(manifest.legacyRoots, legacyExts)
  const locByFile = new Map<string, number>()
  let totalLegacyLoc = 0
  for (const file of allLegacyFiles) {
    const lines = countLines(file)
    locByFile.set(file, lines)
    totalLegacyLoc += lines
  }

  const rustFilesSet = collectFiles(manifest.rustRoots, rustExts)
  let rustLoc = 0
  for (const file of rustFilesSet) {
    rustLoc += countLines(file)
  }

  const sliceById = new Map(manifest.slices.map((s) => [s.id, s]))
  for (const id of extraSlices) {
    if (!sliceById.has(id)) {
      console.error(`Unknown slice ID "${id}". Valid IDs: ${manifest.slices.map((s) => s.id).join(', ')}`)
      process.exit(1)
    }
  }

  const baselineIds = envName
    ? parseFlagEnv(envName, manifest)
    : parseStatusFile(statusPath, includeInReview)
  const baselineSet = new Set(baselineIds)
  const addedSlices = extraSlices.filter((id) => !baselineSet.has(id))
  const afterSet = new Set([...baselineSet, ...addedSlices])

  const collectSuperseded = (ids: Set<string>): Set<string> => {
    const paths: string[] = []
    for (const id of ids) {
      const slice = sliceById.get(id)
      if (slice) paths.push(...slice.legacyPaths)
    }
    const files = collectFiles(paths, legacyExts)
    const intersection = new Set<string>()
    for (const f of files) {
      if (allLegacyFiles.has(f)) intersection.add(f)
    }
    return intersection
  }

  const sumLoc = (files: Set<string>): number => {
    let sum = 0
    for (const f of files) sum += locByFile.get(f) ?? 0
    return sum
  }

  const beforeFiles = collectSuperseded(baselineSet)
  const afterFiles = collectSuperseded(afterSet)
  const beforeSupersededLoc = sumLoc(beforeFiles)
  const afterSupersededLoc = sumLoc(afterFiles)
  const beforePct = pct(beforeSupersededLoc, totalLegacyLoc)
  const afterPct = pct(afterSupersededLoc, totalLegacyLoc)

  const surfaces: SurfaceStats[] = Object.entries(manifest.surfaces).map(([id, def]) => {
    const surfaceFiles = collectFiles(def.paths, legacyExts)
    let totalLoc = 0
    let beforeLoc = 0
    let afterLoc = 0
    for (const f of surfaceFiles) {
      if (!allLegacyFiles.has(f)) continue
      const lines = locByFile.get(f) ?? 0
      totalLoc += lines
      if (beforeFiles.has(f)) beforeLoc += lines
      if (afterFiles.has(f)) afterLoc += lines
    }
    const bPct = pct(beforeLoc, totalLoc)
    const aPct = pct(afterLoc, totalLoc)
    return {
      id,
      label: def.label,
      totalLoc,
      beforeSupersededLoc: beforeLoc,
      afterSupersededLoc: afterLoc,
      beforePct: bPct,
      afterPct: aPct,
      deltaPct: Math.round((aPct - bPct) * 100) / 100
    }
  })

  const remainingLegacyLoc = totalLegacyLoc - afterSupersededLoc
  const report: ProgressReport = {
    baselineSlices: [...baselineSet],
    addedSlices,
    totalLegacyLoc,
    beforeSupersededLoc,
    afterSupersededLoc,
    beforePct,
    afterPct,
    deltaLoc: afterSupersededLoc - beforeSupersededLoc,
    deltaPct: Math.round((afterPct - beforePct) * 100) / 100,
    remainingLegacyLoc,
    rustLoc,
    rustFiles: rustFilesSet.size,
    rustShareOfServedPct: pct(rustLoc, rustLoc + remainingLegacyLoc),
    surfaces
  }

  if (format === 'json') {
    console.log(JSON.stringify(report, null, 2))
    return
  }

  const sliceSummary = addedSlices.length > 0
    ? `Including slice(s): ${addedSlices.map((id) => `\`${id}\``).join(', ')}`
    : 'Current verified baseline (pass `--slice <ID>` to preview PR impact)'

  const lines: string[] = [
    '### Migration Progress (% of Legacy TS/Vue App Superseded)',
    '',
    `> ${sliceSummary}`,
    `> Core App Progress: \`${progressBar(report.afterPct)}\` (${report.afterSupersededLoc.toLocaleString()} / ${report.totalLegacyLoc.toLocaleString()} legacy LOC superseded; ${formatDelta(report.deltaPct, '%')} from this PR)`,
    '',
    '| Scope | Before (Legacy Superseded) | After (Legacy Superseded) | PR Delta |',
    '|---|---:|---:|---:|',
    `| **Entire Core App (Desktop + Muya)** | ${report.beforeSupersededLoc.toLocaleString()} / ${report.totalLegacyLoc.toLocaleString()} LOC (**${report.beforePct.toFixed(2)}%**) | ${report.afterSupersededLoc.toLocaleString()} / ${report.totalLegacyLoc.toLocaleString()} LOC (**${report.afterPct.toFixed(2)}%**) | **${formatDelta(report.deltaPct, '%')}** (+${report.deltaLoc.toLocaleString()} LOC) |`
  ]

  for (const s of report.surfaces) {
    const deltaLoc = s.afterSupersededLoc - s.beforeSupersededLoc
    lines.push(
      `| ${s.label} | ${s.beforeSupersededLoc.toLocaleString()} / ${s.totalLoc.toLocaleString()} LOC (${s.beforePct.toFixed(2)}%) | ${s.afterSupersededLoc.toLocaleString()} / ${s.totalLoc.toLocaleString()} LOC (${s.afterPct.toFixed(2)}%) | ${formatDelta(s.deltaPct, '%')} (+${deltaLoc.toLocaleString()} LOC) |`
    )
  }

  lines.push(
    '',
    `**Language Footprint:** Active Legacy TS/Vue: \`${report.remainingLegacyLoc.toLocaleString()} LOC\` · Superseded Legacy TS/Vue: \`${report.afterSupersededLoc.toLocaleString()} LOC\` · Active Rust: \`${report.rustLoc.toLocaleString()} LOC\` (${report.rustFiles} files; \`${report.rustShareOfServedPct.toFixed(2)}%\` of actively served code)`
  )

  console.log(lines.join('\n'))
}

main()
