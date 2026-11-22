import assert from 'node:assert/strict'
import test from 'node:test'
import { compareReports, parseArgs, settingsCategoryForSlice, type MeasureReport } from './migration-measure.ts'

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

test('compareReports subtracts the legacy baseline', () => {
  const delta = compareReports(report('before', 800, 100), report('after', 900, 120))
  assert.equal(delta.startupLatencyMs, 100)
  assert.equal(delta.memoryRssMb, 20)
})
