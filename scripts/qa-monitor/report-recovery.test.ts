import assert from 'node:assert/strict'
import { execFileSync, spawnSync } from 'node:child_process'
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { delimiter, join } from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

test('filing failures preserve agent output and replay files only missing findings without an API key', (t) => {
  const dir = mkdtempSync(join(tmpdir(), 'qa-recovery-test-'))
  t.after(() => rmSync(dir, { recursive: true, force: true }))
  const root = fileURLToPath(new URL('../..', import.meta.url))
  const script = fileURLToPath(new URL('./run-qa-monitor.ts', import.meta.url))
  const sha = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim()
  const report = join(dir, 'report.json')
  const telemetry = join(dir, 'telemetry.json')
  const issues = join(dir, 'issues.json')
  const mockHook = join(dir, 'hook.mjs')
  const archive = {
    summary: 'Two regressions',
    slicesAudited: ['W2.A'],
    findings: ['Cold paint', 'Lost edit'].map((title) => ({
      title, slice: 'W2.A', severity: 'P1', category: 'performance', evidence: ['Measured in an isolated VM.']
    }))
  }

  // The cloud stub returns exactly one completed audit. It is unavailable during replay.
  writeFileSync(join(dir, 'sdk.mjs'), `
    export class CursorAgentError extends Error {}
    export class Agent {
      static async create() {
        return {
          agentId: 'bc-qa', [Symbol.asyncDispose]() {},
          async send() {
            return {
              id: 'run-qa', async *stream() {},
              async wait() { return { status: 'finished', result: ${JSON.stringify(JSON.stringify(archive))} } }
            }
          }
        }
      }
    }
  `)
  writeFileSync(mockHook, `
    import { registerHooks } from 'node:module'
    registerHooks({ resolve(specifier, ctx, next) {
      if (specifier === '@cursor/sdk') {
        if (!process.env.CURSOR_API_KEY) throw new Error('Unexpected SDK access during replay')
        return { url: new URL('sdk.mjs', import.meta.url).href, shortCircuit: true }
      }
      return next(specifier, ctx)
    } })
  `)

  // A partial GitHub failure creates the first issue, then refuses the second.
  writeFileSync(issues, '[]')
  writeFileSync(join(dir, 'gh'), `#!/usr/bin/env node
    const fs = require('node:fs')
    const args = process.argv.slice(2)
    const rows = JSON.parse(fs.readFileSync(process.env.TEST_ISSUES, 'utf8'))
    if (args[0] === 'api') {
      if (process.env.FAIL_LOOKUP) process.exit(1)
      console.log(JSON.stringify([[], rows]))
    } else if (args[0] === 'label' || args[0] === 'issue' && args[1] === 'create') {
      if (process.env.READ_ONLY_GH) throw new Error('Capture has no GitHub write permission')
      if (!fs.existsSync(process.env.TEST_REPORT)) throw new Error('GitHub mutation before report was saved')
      if (args[0] === 'label') process.exit(0)
      if (rows.length === 1 && process.env.FAIL_SECOND) throw new Error('simulated issue filing failure')
      const number = rows.length + 1
      const html_url = 'https://github.com/test/marktext/issues/' + number
      rows.push({ number, html_url, body: args[args.indexOf('--body') + 1], title: args[args.indexOf('--title') + 1] })
      fs.writeFileSync(process.env.TEST_ISSUES, JSON.stringify(rows))
      console.log(html_url)
    } else {
      throw new Error('Unexpected gh subcommand ' + args[0])
    }
  `, { mode: 0o755 })
  const run = (args: string[], env: NodeJS.ProcessEnv = {}) => spawnSync(process.execPath, [
    '--no-warnings', '--import', mockHook, script, '--repo', 'test/marktext', ...args
  ], { cwd: root, encoding: 'utf8', env: {
    ...process.env, CURSOR_API_KEY: '', TEST_REPORT: report, TEST_ISSUES: issues,
    PATH: [dir, process.env.PATH].join(delimiter), ...env
  } })
  // Capture job receives a read-only token. Filing may be rerun independently by Actions.
  const capture = run(['--slice', 'W2.A', '--report-only', '--report-out', report], {
    CURSOR_API_KEY: 'isolated-test-key', READ_ONLY_GH: '1'
  })
  assert.equal(capture.status, 0, capture.stderr)
  assert.equal(JSON.parse(readFileSync(issues, 'utf8')).length, 0)
  const captured = JSON.parse(readFileSync(report, 'utf8'))
  assert.ok(Date.parse(captured.finishedAt) >= Date.parse(captured.startedAt))
  const capturePreview = run(['--replay', report, '--out', telemetry, '--dry-run'], { READ_ONLY_GH: '1' })
  assert.equal(capturePreview.status, 0, capturePreview.stderr)
  const captureTelemetry = JSON.parse(readFileSync(telemetry, 'utf8'))
  assert.equal(captureTelemetry.durationMs, Date.parse(captured.finishedAt) - Date.parse(captured.startedAt))
  assert.equal(captureTelemetry.startedAt, captured.startedAt)
  assert.equal(captureTelemetry.finishedAt, captured.finishedAt)
  rmSync(report)
  rmSync(telemetry)

  const first = run(['--slice', 'W2.A', '--report-out', report, '--out', telemetry], {
    CURSOR_API_KEY: 'isolated-test-key', FAIL_SECOND: '1'
  })
  assert.notEqual(first.status, 0)
  assert.match(first.stderr, /simulated issue filing failure/)
  assert.equal(existsSync(telemetry), false)
  const original = readFileSync(report, 'utf8')
  const saved = JSON.parse(original)
  assert.equal(saved.headSha, sha)
  assert.deepEqual(saved.targetSlices, ['W2.A'])
  assert.deepEqual(saved.report.findings.map((f: { title: string }) => f.title), ['Cold paint', 'Lost edit'])
  assert.equal(JSON.parse(readFileSync(issues, 'utf8')).length, 1)

  const replay = ['--replay', report, '--out', telemetry]
  for (const extra of [['--repo', 'other/repo'], ['--head-sha', 'HEAD~1'], ['--out', report]]) {
    const invalid = run([...replay, ...extra])
    assert.notEqual(invalid.status, 0)
    assert.match(invalid.stderr, /Saved report must be|must be different files/)
  }
  assert.notEqual(run(replay, { FAIL_LOOKUP: '1' }).status, 0)
  const preview = run([...replay, '--dry-run'])
  assert.equal(preview.status, 0, preview.stderr)
  assert.equal(JSON.parse(readFileSync(issues, 'utf8')).length, 1)
  const retry = run(replay)
  assert.equal(retry.status, 0, retry.stderr)
  const result = JSON.parse(readFileSync(telemetry, 'utf8'))
  assert.equal(result.runId, 'run-qa')
  assert.equal(result.issuesFiled, 1)
  assert.equal(result.duplicatesSkipped, 1)
  assert.deepEqual(result.slicesAudited, saved.report.slicesAudited)
  assert.equal(JSON.parse(readFileSync(issues, 'utf8')).length, 2)
  assert.equal(readFileSync(report, 'utf8'), original)
})
