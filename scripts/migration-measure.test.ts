import assert from 'node:assert/strict'
import test from 'node:test'
import { compareReports, electronLaunchArgs, isPanePainted, parseArgs, settingsCategoryForSlice, type MeasureReport } from './migration-measure.ts'

const report = (phase: MeasureReport['phase'], startup: number, memory: number): MeasureReport => ({
  slice: 'A1',
  phase,
  flag: phase === 'after' ? 'rust-wasm' : null,
  capturedAt: '2026-11-22T00:00:00.000Z',
  startupLatencyMs: startup,
  paneSwitchLatencyMs: 40,
  memoryRssMb: { main: 10, renderer: memory, gpu: 5, total: 15 + memory },
  idleCpuPercent: 1,
  artifactSizeMb: { rendererJs: 7, wasm: null },
  screenshot: null
})

test('parseArgs requires slice, phase, and out', () => {
  assert.throws(() => parseArgs(['--slice', 'A1']))
  const options = parseArgs([
    '--slice', 'A1',
    '--phase', 'after',
    '--out', '/tmp/a1.json',
    '--flag', 'rust-wasm',
    '--compare', '/tmp/before.json',
    '--screenshot', '/tmp/a1.png',
    '--skip-launch'
  ])
  assert.equal(options.slice, 'A1')
  assert.equal(options.phase, 'after')
  assert.equal(options.flag, 'rust-wasm')
  assert.equal(options.skipLaunch, true)
})

test('preference slices open their settings category', () => {
  assert.equal(settingsCategoryForSlice({ id: 'A1', flag: 'prefs.pane.general' }), 'general')
  assert.equal(settingsCategoryForSlice({ id: 'A8', flag: 'prefs.sidebar' }), 'general')
  assert.equal(settingsCategoryForSlice({ id: 'B1', flag: 'dialog.about' }), null)
})

test('pane latency waits for a painted legacy page or a ready wasm canvas', () => {
  assert.equal(isPanePainted({
    visibleFrames: [],
    legacy: { width: 640, height: 480, textLength: 12 }
  }), true)
  assert.equal(isPanePainted({
    visibleFrames: [],
    legacy: { width: 640, height: 480, textLength: 0 }
  }), false)
  assert.equal(isPanePainted({
    visibleFrames: [{ paneReady: null, canvasWidth: 0, canvasHeight: 0 }],
    legacy: null
  }), false)
  assert.equal(isPanePainted({
    visibleFrames: [{ paneReady: 'general', canvasWidth: 650, canvasHeight: 567 }],
    legacy: null
  }), true)
})

test('compareReports subtracts the legacy baseline', () => {
  const delta = compareReports(report('before', 800, 100), report('after', 900, 120))
  assert.equal(delta.startupLatencyMs, 100)
  assert.equal(delta.memoryRssMb, 20)
})

test('electronLaunchArgs uses SwiftShader only on Linux so macOS uses Metal WebGL2', () => {
  const linuxArgs = electronLaunchArgs('/app/desktop', '/tmp/ud', 'linux')
  assert.equal(linuxArgs.includes('--use-angle=swiftshader'), true)
  assert.equal(linuxArgs.includes('--enable-unsafe-swiftshader'), true)

  const macArgs = electronLaunchArgs('/app/desktop', '/tmp/ud', 'darwin')
  assert.equal(macArgs.includes('--use-angle=swiftshader'), false)
  assert.deepEqual(macArgs, ['/app/desktop', '--user-data-dir', '/tmp/ud'])
})
