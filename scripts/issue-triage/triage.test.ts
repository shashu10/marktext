import assert from 'node:assert/strict'
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import test from 'node:test'
import { loadSlicesCatalog } from '../qa-monitor/change-detector.ts'
import { postTriageResult, renderTriageComment } from './commenter.ts'
import {
  buildRoutingLabelSpecs,
  isTriageEligibleLabels,
  loadOwnersConfig,
  resolveOwnerRoute
} from './ownership.ts'
import { buildTriagePrompt } from './prompt.ts'
import {
  buildTriageMarker,
  hasTriageMarker,
  parseTriageAnalysisFromText,
  validateTriageAnalysis
} from './schema.ts'
import {
  buildTriageAgentOptions,
  parseTriageCliArgs,
  readSavedTriageReport
} from './run-triage.ts'

const REPO_ROOT = resolve('.')

test('isTriageEligibleLabels filters bug/regression issues and honors triage:skip', () => {
  assert.equal(isTriageEligibleLabels(['bug']).eligible, true)
  assert.equal(isTriageEligibleLabels(['Regression', 'severity:P2']).eligible, true)
  assert.equal(isTriageEligibleLabels(['enhancement', 'docs']).eligible, false)
  assert.equal(isTriageEligibleLabels(['bug', 'triage:skip']).eligible, false)
})

test('resolveOwnerRoute routes deterministically by slice override, shared path, surface, and fallback', () => {
  const { slices } = loadSlicesCatalog(REPO_ROOT)
  const owners = loadOwnersConfig(REPO_ROOT)

  // 1. Slice override (W2.A -> team:native-runtime)
  const nativeRoute = resolveOwnerRoute({
    affectedSliceIds: ['W2.A'],
    suspectedFiles: [],
    slicesCatalog: slices,
    owners
  })
  assert.equal(nativeRoute.team, 'team:native-runtime')
  assert.equal(nativeRoute.matchedBy, 'slice')

  // 2. Shared path rule (marktext-ui-kit -> team:design-systems)
  const uiKitRoute = resolveOwnerRoute({
    affectedSliceIds: ['A1', 'A2'],
    suspectedFiles: ['rust/crates/marktext-ui-kit/src/theme.rs'],
    slicesCatalog: slices,
    owners
  })
  assert.equal(uiKitRoute.team, 'team:design-systems')
  assert.equal(uiKitRoute.matchedBy, 'shared-path')
  assert.deepEqual(uiKitRoute.matchedSlices.map((s) => s.id), ['A1', 'A2'])

  // 3. Surface rule (A1 -> preferences -> team:prefs-migration)
  const prefsRoute = resolveOwnerRoute({
    affectedSliceIds: ['A1'],
    suspectedFiles: ['rust/crates/mt-pref-general/src/lib.rs'],
    slicesCatalog: slices,
    owners
  })
  assert.equal(prefsRoute.team, 'team:prefs-migration')
  assert.equal(prefsRoute.matchedBy, 'surface')
  assert.equal(prefsRoute.demoMode, true)

  // 4. Fallback when nothing matches
  const fallbackRoute = resolveOwnerRoute({
    affectedSliceIds: [],
    suspectedFiles: ['scripts/unknown.ts'],
    slicesCatalog: slices,
    owners
  })
  assert.equal(fallbackRoute.team, 'team:migration-platform')
  assert.equal(fallbackRoute.matchedBy, 'fallback')

  const labels = buildRoutingLabelSpecs(prefsRoute)
  assert.ok(labels.every((l) => l.description.length <= 100))
  assert.deepEqual(labels.map((l) => l.name), ['triage:complete', 'team:prefs-migration', 'slice:A1'])
})

test('parseTriageAnalysisFromText validates structured JSON and rejects invalid confidence', () => {
  const output = `
Investigated issue #18 using git log and source inspection.

\`\`\`triage-report-json
{
  "summary": "GPUI panes remain in light theme because marktext-wasm-host hardcodes Theme::light.",
  "affectedSlices": ["A1", "A2"],
  "suspectedFiles": ["rust/crates/marktext-wasm-host/src/lib.rs"],
  "suspectCommits": [
    {
      "sha": "4b3f3c0f5d26",
      "prNumber": 5,
      "reason": "Initial WASM host initialized GPUI theme without syncing Electron theme preference."
    }
  ],
  "rootCauseHypothesis": "marktext-wasm-host initializes gpui_components::Theme to light mode and never applies PREFERENCES.theme.",
  "reproHint": "Switch MarkText to Dark theme, then open Preferences with flag=rust-wasm.",
  "suggestedNextStep": "Sync gpui_components::Theme when PREFERENCES payload arrives in marktext-wasm-host.",
  "confidence": "high",
  "evidence": ["rust/crates/marktext-wasm-host/src/lib.rs initializes Light theme unconditionally."]
}
\`\`\`
`
  const parsed = parseTriageAnalysisFromText(output)
  assert.equal(parsed.confidence, 'high')
  assert.deepEqual(parsed.affectedSlices, ['A1', 'A2'])
  assert.equal(parsed.suspectCommits[0]?.prNumber, 5)

  assert.throws(() =>
    validateTriageAnalysis({
      summary: 'Bad confidence',
      rootCauseHypothesis: 'Missing',
      confidence: 'certain'
    })
  )
})

test('renderTriageComment embeds idempotent marker and supports dry-run posting', () => {
  const { slices } = loadSlicesCatalog(REPO_ROOT)
  const owners = loadOwnersConfig(REPO_ROOT)
  const analysis = validateTriageAnalysis({
    summary: 'Dark theme is not propagated to GPUI panes.',
    affectedSlices: ['A1'],
    suspectedFiles: ['rust/crates/marktext-wasm-host/src/lib.rs'],
    suspectCommits: [{ sha: '4b3f3c0f', prNumber: 5, reason: 'Hardcoded Light theme.' }],
    rootCauseHypothesis: 'Theme payload is ignored by wasm host.',
    reproHint: 'Enable dark mode and open General settings.',
    suggestedNextStep: 'Map PREFERENCES.theme to GPUI theme palette.',
    confidence: 'high',
    evidence: ['Verified in marktext-wasm-host/src/lib.rs']
  })
  const route = resolveOwnerRoute({
    affectedSliceIds: analysis.affectedSlices,
    suspectedFiles: analysis.suspectedFiles,
    slicesCatalog: slices,
    owners
  })

  const meta = {
    repoSlug: 'shashu10/marktext',
    headSha: '4b3f3c0f5d2603afe5f93c1140a9a718492e0250',
    issueNumber: 18,
    agentId: 'local-triage-1',
    runId: 'run-triage-1',
    runtime: 'local' as const,
    dryRun: true
  }

  const body = renderTriageComment(analysis, route, meta)
  assert.equal(hasTriageMarker(body), true)
  assert.match(body, new RegExp(buildTriageMarker(18, meta.headSha).replace(/[.*+?^${}()|[\]\\]/g, '\\$&')))
  assert.match(body, /team:prefs-migration/)
  assert.match(body, /single-maintainer demo mode/)
  assert.match(body, /4b3f3c0f/)

  const posted = postTriageResult({ repoRoot: REPO_ROOT, analysis, route, meta })
  assert.equal(posted.status, 'dry-run')
  assert.ok(posted.appliedLabels.includes('team:prefs-migration'))
  assert.ok(posted.appliedLabels.includes('triage:complete'))
})

test('buildTriagePrompt enforces read-only non-modifying charter', () => {
  const { slices } = loadSlicesCatalog(REPO_ROOT)
  const prompt = buildTriagePrompt({
    repoSlug: 'shashu10/marktext',
    headSha: '4b3f3c0f5d26',
    issue: {
      number: 18,
      title: '[P2][A1-A7] Dark theme mismatch',
      body: 'Panes stay light in dark mode.',
      labels: ['regression', 'severity:P2'],
      author: 'github-actions[bot]',
      url: 'https://github.com/shashu10/marktext/issues/18',
      state: 'open'
    },
    slices,
    recentCommits: []
  })

  assert.match(prompt, /DO NOT FIX THE BUG/)
  assert.match(prompt, /DO NOT INVENT OWNER HANDLES/)
  assert.match(prompt, /triage-report-json/)
})

test('parseTriageCliArgs, buildTriageAgentOptions, and saved report replay validation', () => {
  const localOpts = parseTriageCliArgs(['--issue', '18', '--dry-run'], REPO_ROOT)
  assert.equal(localOpts.runtime, 'local')
  assert.equal(localOpts.issueNumber, 18)

  const sdkLocal = buildTriageAgentOptions({
    options: localOpts,
    repoRoot: REPO_ROOT,
    headSha: '4b3f3c0f5d26',
    issueNumber: 18,
    apiKey: 'test-key'
  })
  assert.equal(sdkLocal.cloud, undefined)
  assert.equal(sdkLocal.local?.cwd, REPO_ROOT)
  assert.deepEqual(sdkLocal.disallowedTools, ['edit', 'delete'])

  const cloudOpts = parseTriageCliArgs(['--issue', '18', '--runtime', 'cloud'], REPO_ROOT)
  const sdkCloud = buildTriageAgentOptions({
    options: cloudOpts,
    repoRoot: REPO_ROOT,
    headSha: '4b3f3c0f5d26',
    issueNumber: 18,
    apiKey: 'test-key'
  })
  assert.equal(sdkCloud.cloud?.autoCreatePR, false)
  assert.equal(sdkCloud.cloud?.skipReviewerRequest, true)
  assert.equal(sdkCloud.idempotencyKey, 'issue-triage-18-4b3f3c0')

  const tmpDir = mkdtempSync(join(tmpdir(), 'triage-test-'))
  try {
    const savedPath = join(tmpDir, 'report.json')
    writeFileSync(savedPath, JSON.stringify({
      version: 1,
      startedAt: '2026-11-24T12:00:00.000Z',
      finishedAt: '2026-11-24T12:01:00.000Z',
      repoSlug: 'shashu10/marktext',
      headSha: '4b3f3c0f5d26',
      issueNumber: 18,
      runtime: 'local',
      agentId: 'local-1',
      runId: 'run-1',
      analysis: {
        summary: 'Summary',
        affectedSlices: ['A1'],
        suspectedFiles: ['rust/crates/marktext-wasm-host/src/lib.rs'],
        suspectCommits: [],
        rootCauseHypothesis: 'Hypothesis',
        reproHint: 'Repro',
        suggestedNextStep: 'Fix',
        confidence: 'medium',
        evidence: []
      }
    }))
    const loaded = readSavedTriageReport(savedPath, 'shashu10/marktext', 18)
    assert.equal(loaded.issueNumber, 18)
    assert.throws(() => readSavedTriageReport(savedPath, 'shashu10/marktext', 99))
  } finally {
    rmSync(tmpDir, { recursive: true, force: true })
  }
})
