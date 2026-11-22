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
  idleCpuPercent: number | null
  artifactSizeMb: { rendererJs: number; wasm: number | null } | null
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
  return {
    rendererJs: Number((rendererBytes / (1024 * 1024)).toFixed(3)),
    wasm: wasmBytes === 0 ? null : Number((wasmBytes / (1024 * 1024)).toFixed(3))
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

const launchReport = async(options: CliOptions, slice: SliceDef): Promise<MeasureReport> => {
  const playwright = await import('playwright')
  const started = Date.now()
  const userDataDir = fs.mkdtempSync(path.join('/tmp', 'marktext-measure-'))
  const env: Record<string, string> = {}
  for (const [key, value] of Object.entries(process.env)) {
    if (value !== undefined) env[key] = value
  }
  env.MARKTEXT_MIGRATION_ENV = 'prod'
  if (options.flag) env.MARKTEXT_MIGRATION_OVERRIDES = `${slice.flag}=${options.flag}`

  const app = await playwright._electron.launch({
    executablePath: electronPath(),
    // Xvfb has no hardware GPU. GPUI's web backend needs WebGL2, which Chromium
    // only provides here through SwiftShader.
    args: [
      '--ignore-gpu-blocklist',
      '--enable-unsafe-swiftshader',
      '--use-gl=angle',
      '--use-angle=swiftshader',
      path.join(repoRoot, 'packages/desktop'),
      '--user-data-dir',
      userDataDir
    ],
    cwd: path.join(repoRoot, 'packages/desktop'),
    env,
    timeout: 30000
  })
  try {
    const page = await app.firstWindow()
    await page.waitForLoadState('domcontentloaded')
    const startupLatencyMs = Date.now() - started
    const category = settingsCategoryForSlice(slice)
    let paneSwitchLatencyMs: number | null = null
    let shot = page
    if (category) {
      const paneStarted = Date.now()
      await app.evaluate(({ ipcMain }, requested: string) => {
        ipcMain.emit('app-create-settings-window', requested)
      }, category)
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
    }
    if (options.screenshot) {
      fs.mkdirSync(path.dirname(path.resolve(options.screenshot)), { recursive: true })
      await shot.screenshot({ path: options.screenshot })
    }
    await app.evaluate(({ app: electronApp }) => electronApp.getAppMetrics())
    await page.waitForTimeout(5000)
    const metrics = await app.evaluate(({ app: electronApp }) => electronApp.getAppMetrics()) as ProcessMetric[]
    const cpu = metrics.reduce((sum, metric) => sum + metric.cpu.percentCPUUsage, 0)
    return {
      slice: options.slice,
      phase: options.phase,
      flag: options.flag,
      capturedAt: new Date().toISOString(),
      startupLatencyMs,
      paneSwitchLatencyMs,
      memoryRssMb: rssMb(metrics),
      idleCpuPercent: Number(cpu.toFixed(3)),
      artifactSizeMb: artifactSizes(),
      screenshot: options.screenshot
    }
  } finally {
    await app.evaluate(({ app: electronApp }) => electronApp.exit(0)).catch(() => {})
    await app.close().catch(() => {})
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
