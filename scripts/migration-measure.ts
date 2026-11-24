import { spawnSync } from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'
import { pathToFileURL } from 'node:url'

export interface MeasureReport {
  slice: string
  phase: 'before' | 'after'
  flag: string | null
  capturedAt: string
  startupLatencyMs: number | null
  paneSwitchLatencyMs: number | null
  memoryRssMb: { main: number; renderer: number; gpu: number; total: number } | null
  /** Stage 2 sidecar RSS when `window.preferences=gpui-native`. */
  nativeChildRssMb: number | null
  idleCpuPercent: number | null
  artifactSizeMb: { rendererJs: number; wasm: number | null; nativePrefApp: number | null } | null
  screenshot: string | null
}

interface SliceDef {
  id: string
  flag: string
}

interface CliOptions {
  slice: string
  phase: 'before' | 'after'
  out: string
  screenshot: string | null
  flag: string | null
  compare: string | null
  skipLaunch: boolean
}

const repoRoot = path.resolve(import.meta.dirname, '..')

const usage = (): string => `Usage: pnpm migration:measure --slice <id> --phase before|after --out <file.json> [--screenshot <file.png>] [--flag <mode>] [--compare <before.json>] [--skip-launch]`

export const parseArgs = (argv: string[]): CliOptions => {
  const values = new Map<string, string | boolean>()
  for (let i = 0; i < argv.length; i++) {
    const token = argv[i]
    if (!token?.startsWith('--')) continue
    const key = token.slice(2)
    const next = argv[i + 1]
    if (!next || next.startsWith('--')) {
      values.set(key, true)
      continue
    }
    values.set(key, next)
    i++
  }
  const slice = values.get('slice')
  const phase = values.get('phase')
  const out = values.get('out')
  if (typeof slice !== 'string' || typeof out !== 'string' || (phase !== 'before' && phase !== 'after')) {
    throw new Error(usage())
  }
  const flag = values.get('flag')
  const screenshot = values.get('screenshot')
  const compare = values.get('compare')
  return {
    slice,
    phase,
    out,
    screenshot: typeof screenshot === 'string' ? screenshot : null,
    flag: typeof flag === 'string' ? flag : null,
    compare: typeof compare === 'string' ? compare : null,
    skipLaunch: values.get('skip-launch') === true
  }
}

const readSlices = (): SliceDef[] => {
  const raw = fs.readFileSync(path.join(repoRoot, 'docs/migration/slices.json'), 'utf8')
  const parsed = JSON.parse(raw) as { slices: SliceDef[] }
  return parsed.slices
}

export interface PaneProbe {
  visibleFrames: Array<{ paneReady: string | null; canvasWidth: number; canvasHeight: number }>
  legacy: { width: number; height: number; textLength: number } | null
}

/** A visible wasm frame counts only after `PANE_CHANGED` and a sized canvas. */
export const isPanePainted = (probe: PaneProbe): boolean => {
  if (probe.visibleFrames.length > 0) {
    return probe.visibleFrames.every(frame =>
      frame.paneReady !== null &&
      frame.paneReady.length > 0 &&
      frame.canvasWidth > 0 &&
      frame.canvasHeight > 0
    )
  }
  return probe.legacy !== null &&
    probe.legacy.width > 0 &&
    probe.legacy.height > 0 &&
    probe.legacy.textLength > 0
}

export const settingsCategoryForSlice = (slice: SliceDef): string | null => {
  if (slice.flag.startsWith('prefs.pane.')) return slice.flag.slice('prefs.pane.'.length)
  if (slice.flag === 'prefs.sidebar' || slice.flag === 'window.preferences') return 'general'
  return null
}

const directoryBytes = (dir: string, extension: string): number => {
  if (!fs.existsSync(dir)) return 0
  let total = 0
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name)
    if (entry.isDirectory()) total += directoryBytes(full, extension)
    else if (entry.name.endsWith(extension)) total += fs.statSync(full).size
  }
  return total
}

export const artifactSizes = (): MeasureReport['artifactSizeMb'] => {
  const rendererBytes = directoryBytes(
    path.join(repoRoot, 'packages/desktop/out/renderer/assets'),
    '.js'
  )
  const wasmBytes = directoryBytes(
    path.join(repoRoot, 'rust/target/wasm32-unknown-unknown/release'),
    '.wasm'
  )
  const nativeBinary = [
    path.join(repoRoot, 'rust/target/release/mt-pref-app'),
    path.join(repoRoot, 'rust/target/debug/mt-pref-app')
  ].find(candidate => fs.existsSync(candidate))
  const nativeBytes = nativeBinary ? fs.statSync(nativeBinary).size : 0
  return {
    rendererJs: Number((rendererBytes / (1024 * 1024)).toFixed(3)),
    wasm: wasmBytes === 0 ? null : Number((wasmBytes / (1024 * 1024)).toFixed(3)),
    nativePrefApp: nativeBytes === 0 ? null : Number((nativeBytes / (1024 * 1024)).toFixed(3))
  }
}

/** W2.A Stage 1 comparison needs every prefs.* pane on rust-wasm, not window.preferences. */
export const migrationOverridesFor = (slice: SliceDef, flag: string | null): string | null => {
  if (!flag) return null
  if (slice.id === 'W2.A' && flag === 'rust-wasm') {
    return [
      'window.preferences=legacy',
      'prefs.sidebar=rust-wasm',
      'prefs.pane.general=rust-wasm',
      'prefs.pane.appearance=rust-wasm',
      'prefs.pane.editor=rust-wasm',
      'prefs.pane.files=rust-wasm',
      'prefs.pane.markdown=rust-wasm',
      'prefs.pane.images=rust-wasm',
      'prefs.pane.keybindings=rust-wasm'
    ].join(',')
  }
  return `${slice.flag}=${flag}`
}

const readProcessRssMb = (pid: number): number | null => {
  try {
    const status = fs.readFileSync(`/proc/${pid}/status`, 'utf8')
    const match = /^VmRSS:\s+(\d+)\s+kB$/m.exec(status)
    if (!match) return null
    return Number((Number(match[1]) / 1024).toFixed(3))
  } catch {
    return null
  }
}

const readProcPpid = (pid: number): number | null => {
  try {
    const status = fs.readFileSync(`/proc/${pid}/status`, 'utf8')
    const match = /^PPid:\s+(\d+)$/m.exec(status)
    return match ? Number(match[1]) : null
  } catch {
    return null
  }
}

/** True when `pid` is `ancestorPid` or a descendant of it. */
export const isDescendantOf = (pid: number, ancestorPid: number): boolean => {
  let current: number | null = pid
  const seen = new Set<number>()
  while (current != null && current > 0 && !seen.has(current)) {
    if (current === ancestorPid) return true
    seen.add(current)
    current = readProcPpid(current)
  }
  return false
}

/**
 * Locate a live `mt-pref-app`. When `parentPid` is set, only accept processes
 * under that Electron tree so a parallel harness cannot steal another run's
 * sidecar (or leave orphans after the wrong kill).
 */
export const findMtPrefAppPid = (parentPid?: number | null): number | null => {
  try {
    const procs = fs.readdirSync('/proc').filter(name => /^\d+$/.test(name))
    let best: { pid: number; rss: number } | null = null
    for (const id of procs) {
      try {
        const cmdline = fs.readFileSync(`/proc/${id}/cmdline`, 'utf8')
        if (!cmdline.includes('mt-pref-app')) continue
        const pid = Number(id)
        if (parentPid != null && !isDescendantOf(pid, parentPid)) continue
        const status = fs.readFileSync(`/proc/${pid}/status`, 'utf8')
        if (/^State:\s+Z/m.test(status)) continue
        const match = /^VmRSS:\s+(\d+)\s+kB$/m.exec(status)
        const rss = match ? Number(match[1]) : 0
        if (!best || rss > best.rss) best = { pid, rss }
      } catch {
        // process vanished
      }
    }
    return best?.pid ?? null
  } catch {
    return null
  }
}

/** utime+stime jiffies from `/proc/<pid>/stat` (fields 14–15). */
export const readProcessCpuJiffies = (pid: number): number | null => {
  try {
    const stat = fs.readFileSync(`/proc/${pid}/stat`, 'utf8')
    const close = stat.lastIndexOf(')')
    if (close < 0) return null
    const rest = stat.slice(close + 2).split(' ')
    const utime = Number(rest[11])
    const stime = Number(rest[12])
    if (!Number.isFinite(utime) || !Number.isFinite(stime)) return null
    return utime + stime
  } catch {
    return null
  }
}

const cpuJiffiesToPercent = (before: number, after: number, elapsedMs: number): number => {
  const hz = 100
  const seconds = Math.max(elapsedMs / 1000, 0.001)
  return Number((((after - before) / hz / seconds) * 100).toFixed(3))
}

const captureRootScreenshot = (screenshot: string): void => {
  fs.mkdirSync(path.dirname(path.resolve(screenshot)), { recursive: true })
  const attempt = spawnSync('import', ['-window', 'root', screenshot], {
    encoding: 'utf8',
    timeout: 8000
  })
  if (attempt.status === 0) return
  const scrot = spawnSync('scrot', [screenshot], { encoding: 'utf8', timeout: 8000 })
  if (scrot.status !== 0) {
    throw new Error(`native screenshot failed: ${attempt.stderr || scrot.stderr || 'no import/scrot'}`)
  }
}

const killMtPrefAppChildren = (parentPid?: number | null): void => {
  for (let attempt = 0; attempt < 5; attempt++) {
    const pid = findMtPrefAppPid(parentPid)
    if (pid == null) return
    try {
      process.kill(pid, attempt < 2 ? 'SIGTERM' : 'SIGKILL')
    } catch {
      return
    }
  }
}

const numberDelta = (before: number | null, after: number | null): number | null => {
  if (before === null || after === null) return null
  return Number((after - before).toFixed(3))
}

export const compareReports = (before: MeasureReport, after: MeasureReport): Record<string, number | null> => ({
  startupLatencyMs: numberDelta(before.startupLatencyMs, after.startupLatencyMs),
  paneSwitchLatencyMs: numberDelta(before.paneSwitchLatencyMs, after.paneSwitchLatencyMs),
  memoryRssMb: numberDelta(before.memoryRssMb?.total ?? null, after.memoryRssMb?.total ?? null),
  idleCpuPercent: numberDelta(before.idleCpuPercent, after.idleCpuPercent),
  rendererJsMb: numberDelta(before.artifactSizeMb?.rendererJs ?? null, after.artifactSizeMb?.rendererJs ?? null)
})

interface ProcessMetric {
  type: string
  memory: { workingSetSize: number }
  cpu: { percentCPUUsage: number }
}

const rssMb = (metrics: ProcessMetric[]): MeasureReport['memoryRssMb'] => {
  const bucket = (type: string): number => metrics
    .filter(metric => metric.type === type)
    .reduce((sum, metric) => sum + metric.memory.workingSetSize, 0) / 1024
  const main = bucket('Browser')
  const renderer = bucket('Tab')
  const gpu = bucket('GPU')
  return {
    main: Number(main.toFixed(3)),
    renderer: Number(renderer.toFixed(3)),
    gpu: Number(gpu.toFixed(3)),
    total: Number((main + renderer + gpu).toFixed(3))
  }
}

const electronPath = (): string => {
  const desktop = path.join(repoRoot, 'packages/desktop')
  const pathTxt = path.join(desktop, 'node_modules/electron/path.txt')
  const rel = fs.readFileSync(pathTxt, 'utf8').trim()
  return path.join(desktop, 'node_modules/electron/dist', rel)
}

/**
 * Headless Linux (Xvfb) has no hardware GPU, so GPUI's WebGL2 backend needs
 * SwiftShader there. macOS (Metal) and Windows have native hardware WebGL2, and
 * forcing SwiftShader on Apple Silicon breaks canvas initialization.
 */
export const electronLaunchArgs = (
  desktopDir: string,
  userDataDir: string,
  platform: NodeJS.Platform = process.platform
): string[] => {
  const gpuArgs = platform === 'linux'
    ? [
      '--ignore-gpu-blocklist',
      '--enable-unsafe-swiftshader',
      '--use-gl=angle',
      '--use-angle=swiftshader'
    ]
    : []
  return [...gpuArgs, desktopDir, '--user-data-dir', userDataDir]
}

const launchReport = async(options: CliOptions, slice: SliceDef): Promise<MeasureReport> => {
  const playwright = await import('playwright')
  const started = Date.now()
  const userDataDir = fs.mkdtempSync(path.join('/tmp', 'marktext-measure-'))
  const env: Record<string, string> = {}
  for (const [key, value] of Object.entries(process.env)) {
    if (value !== undefined) env[key] = value
  }
  env.MARKTEXT_MIGRATION_ENV = 'prod'
  const overrides = migrationOverridesFor(slice, options.flag)
  if (overrides) env.MARKTEXT_MIGRATION_OVERRIDES = overrides
  if (options.flag === 'gpui-native') {
    env.VK_ICD_FILENAMES = env.VK_ICD_FILENAMES || '/usr/share/vulkan/icd.d/lvp_icd.json'
    const binary = [
      path.join(repoRoot, 'rust/target/release/mt-pref-app'),
      path.join(repoRoot, 'rust/target/debug/mt-pref-app')
    ].find(candidate => fs.existsSync(candidate))
    if (binary) env.MARKTEXT_PREF_APP = binary
  }

  const desktopDir = path.join(repoRoot, 'packages/desktop')
  const nativeMode = options.flag === 'gpui-native' && slice.flag === 'window.preferences'
  const app = await playwright._electron.launch({
    executablePath: electronPath(),
    args: electronLaunchArgs(desktopDir, userDataDir),
    cwd: desktopDir,
    env,
    timeout: 30000
  })
  const electronPid = app.process()?.pid ?? null
  try {
    const page = await app.firstWindow()
    await page.waitForLoadState('domcontentloaded')
    const startupLatencyMs = Date.now() - started
    const category = settingsCategoryForSlice(slice)
    let paneSwitchLatencyMs: number | null = null
    let shot = page
    let nativeChildRssMb: number | null = null
    let sidecarCpuBefore: number | null = null
    let sidecarSampleStarted = 0
    if (category) {
      const paneStarted = Date.now()
      await app.evaluate(({ ipcMain }, requested: string) => {
        ipcMain.emit('app-create-settings-window', requested)
      }, category)
      if (nativeMode) {
        const deadline = Date.now() + 20000
        let pid: number | null = null
        while (Date.now() < deadline) {
          pid = findMtPrefAppPid(electronPid)
          if (pid != null) break
          await page.waitForTimeout(200)
        }
        if (pid == null) throw new Error(`mt-pref-app did not start for ${slice.id}`)
        // Give the first GPUI frame time under lavapipe before capturing.
        await page.waitForTimeout(1500)
        paneSwitchLatencyMs = Date.now() - paneStarted
        if (options.screenshot) captureRootScreenshot(options.screenshot)
        nativeChildRssMb = readProcessRssMb(pid)
        sidecarCpuBefore = readProcessCpuJiffies(pid)
        sidecarSampleStarted = Date.now()
      } else {
        const deadline = Date.now() + 15000
        let settings: typeof page | null = null
        while (Date.now() < deadline) {
          const pages = app.windows()
          if (pages.length >= 2) {
            settings = pages[pages.length - 1] ?? null
            break
          }
          await page.waitForTimeout(200)
        }
        if (!settings) {
          throw new Error(`settings window did not open for ${slice.id}`)
        }
        shot = settings
        await shot.waitForLoadState('domcontentloaded')
        const paintDeadline = Date.now() + 20000
        let painted = false
        while (Date.now() < paintDeadline) {
          const probe = await shot.evaluate(() => {
            const visibleFrames = [...document.querySelectorAll('iframe.gpui-pane-frame')].flatMap(node => {
              const frame = node as HTMLIFrameElement
              const box = frame.getBoundingClientRect()
              if (box.width <= 2 || box.height <= 2) return []
              const canvas = frame.contentDocument?.querySelector('canvas')
              return [{
                paneReady: frame.getAttribute('data-pane-ready'),
                canvasWidth: canvas?.width ?? 0,
                canvasHeight: canvas?.height ?? 0
              }]
            })
            const legacyNode = document.querySelector('.pref-setting')
            const legacyBox = legacyNode?.getBoundingClientRect()
            const legacy = legacyBox && visibleFrames.length === 0
              ? {
                width: legacyBox.width,
                height: legacyBox.height,
                textLength: (legacyNode.textContent ?? '').trim().length
              }
              : null
            return { visibleFrames, legacy }
          })
          if (isPanePainted(probe)) {
            painted = true
            break
          }
          await shot.waitForTimeout(100)
        }
        if (!painted) throw new Error(`pane did not paint for ${slice.id}`)
        paneSwitchLatencyMs = Date.now() - paneStarted
        if (options.screenshot) {
          fs.mkdirSync(path.dirname(path.resolve(options.screenshot)), { recursive: true })
          await shot.screenshot({ path: options.screenshot })
        }
      }
    } else if (options.screenshot) {
      fs.mkdirSync(path.dirname(path.resolve(options.screenshot)), { recursive: true })
      await shot.screenshot({ path: options.screenshot })
    }
    await app.evaluate(({ app: electronApp }) => electronApp.getAppMetrics())
    await page.waitForTimeout(nativeMode ? 2000 : 5000)
    const metrics = await app.evaluate(({ app: electronApp }) => electronApp.getAppMetrics()) as ProcessMetric[]
    let cpu = metrics.reduce((sum, metric) => sum + metric.cpu.percentCPUUsage, 0)
    if (nativeMode) {
      const pid = findMtPrefAppPid(electronPid)
      if (pid != null) {
        nativeChildRssMb = readProcessRssMb(pid)
        const afterJiffies = readProcessCpuJiffies(pid)
        if (
          sidecarCpuBefore != null &&
          afterJiffies != null &&
          sidecarSampleStarted > 0
        ) {
          cpu += cpuJiffiesToPercent(
            sidecarCpuBefore,
            afterJiffies,
            Date.now() - sidecarSampleStarted
          )
        }
      }
    }
    const report: MeasureReport = {
      slice: options.slice,
      phase: options.phase,
      flag: options.flag,
      capturedAt: new Date().toISOString(),
      startupLatencyMs,
      paneSwitchLatencyMs,
      memoryRssMb: rssMb(metrics),
      nativeChildRssMb,
      idleCpuPercent: Number(cpu.toFixed(3)),
      artifactSizeMb: artifactSizes(),
      screenshot: options.screenshot
    }
    // Reap only this Electron tree's sidecar before close() can hang on it.
    killMtPrefAppChildren(electronPid)
    return report
  } finally {
    killMtPrefAppChildren(electronPid)
    try {
      const proc = app.process()
      if (proc?.pid) process.kill(proc.pid, 'SIGKILL')
    } catch {
      // already exited
    }
    await Promise.race([
      app.close().catch(() => {}),
      new Promise<void>(resolve => setTimeout(resolve, 3000))
    ])
    killMtPrefAppChildren(electronPid)
    fs.rmSync(userDataDir, { recursive: true, force: true })
  }
}

const emptyReport = (options: CliOptions): MeasureReport => ({
  slice: options.slice,
  phase: options.phase,
  flag: options.flag,
  capturedAt: new Date().toISOString(),
  startupLatencyMs: null,
  paneSwitchLatencyMs: null,
  memoryRssMb: null,
  nativeChildRssMb: null,
  idleCpuPercent: null,
  artifactSizeMb: artifactSizes(),
  screenshot: options.screenshot
})

const main = async(): Promise<void> => {
  const options = parseArgs(process.argv.slice(2))
  const slice = readSlices().find(item => item.id === options.slice)
  if (!slice) throw new Error(`Unknown slice ${options.slice}`)
  const report = options.skipLaunch ? emptyReport(options) : await launchReport(options, slice)
  const outPath = path.resolve(options.out)
  fs.mkdirSync(path.dirname(outPath), { recursive: true })
  fs.writeFileSync(outPath, `${JSON.stringify(report, null, 2)}\n`)
  console.log(JSON.stringify(report, null, 2))
  if (options.compare) {
    const before = JSON.parse(fs.readFileSync(path.resolve(options.compare), 'utf8')) as MeasureReport
    console.log(JSON.stringify({ delta: compareReports(before, report) }, null, 2))
  }
}

const invokedDirectly = process.argv[1] !== undefined &&
  pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url

if (invokedDirectly) {
  main().catch((error: unknown) => {
    console.error(error instanceof Error ? error.message : error)
    process.exit(1)
  })
}
