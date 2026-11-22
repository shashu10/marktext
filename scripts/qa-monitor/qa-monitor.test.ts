import assert from 'node:assert/strict'
import { resolve } from 'node:path'
import test from 'node:test'
import {
  extractPrNumberFromSubject,
  isRelevantMigrationPath,
  matchSlicesForFiles,
  parseStatusActiveSliceIds,
  type SliceMetadata
} from './change-detector.ts'
import {
  extractFingerprintFromBody,
  fileAuditFindings,
  labelsForFinding,
  renderIssueBody,
  renderIssueTitle,
  type ExistingRegressionIssue
} from './issue-filer.ts'
import { buildQaAuditPrompt } from './prompt.ts'
import {
  normalizeFingerprint,
  parseAuditReportFromText,
  validateAuditReport,
  type QaAuditReport
} from './rubric.ts'
import { buildAgentOptions, parseCliArgs } from './run-qa-monitor.ts'

const SLICE_A1: SliceMetadata = {
  id: 'A1',
  name: 'Preferences — General Pane',
  surface: 'preferences',
  flag: 'prefs.pane.general',
  rustCrate: 'rust/crates/mt-pref-general',
  legacyPaths: ['packages/desktop/src/renderer/src/prefComponents/general/index.vue']
}

const SLICE_A2: SliceMetadata = {
  id: 'A2',
  name: 'Preferences — Appearance & Theme Pane',
  surface: 'preferences',
  flag: 'prefs.pane.appearance',
  rustCrate: 'rust/crates/mt-pref-appearance',
  legacyPaths: ['packages/desktop/src/renderer/src/prefComponents/theme']
}

const SAMPLE_SLICES: SliceMetadata[] = [SLICE_A1, SLICE_A2]

test('normalizeFingerprint prefixes slice and produces stable kebab slugs', () => {
  assert.equal(normalizeFingerprint('A1', 'Cannot Scroll Pane!'), 'a1:cannot-scroll-pane')
  assert.equal(normalizeFingerprint('A1', 'a1:already-prefixed'), 'a1:already-prefixed')
  assert.equal(normalizeFingerprint('A1', '   '), 'a1:unnamed-regression')
})

test('validateAuditReport sorts findings P0 -> P3 and rejects invalid severities', () => {
  const report = validateAuditReport({
    summary: 'Found 4 issues across severities.',
    slicesAudited: ['A1'],
    findings: [
      {
        title: 'Button border radius slightly sharper',
        severity: 'P3',
        slice: 'A1',
        category: 'visual-polish',
        fingerprint: 'button-border-radius'
      },
      {
        title: 'General pane renders black screen',
        severity: 'P0',
        slice: 'A1',
        category: 'blank-or-crash',
        fingerprint: 'black-screen'
      },
      {
        title: 'Language dropdown replaced with button list',
        severity: 'P2',
        slice: 'A1',
        category: 'component-divergence',
        fingerprint: 'dropdown-as-buttons'
      },
      {
        title: 'Cannot scroll General preferences pane',
        severity: 'P1',
        slice: 'A1',
        category: 'interaction-or-scroll',
        fingerprint: 'cannot-scroll',
        workaround: 'Resize preferences window vertically'
      }
    ]
  })

  assert.deepEqual(
    report.findings.map((f) => f.severity),
    ['P0', 'P1', 'P2', 'P3']
  )
  assert.equal(report.findings[1]?.workaround, 'Resize preferences window vertically')

  assert.throws(() =>
    validateAuditReport({
      summary: 'bad',
      findings: [{ title: 'Oops', severity: 'P9', slice: 'A1' }]
    })
  )
})

test('parseAuditReportFromText extracts qa-findings-json and falls back to json block', () => {
  const tagged = `
Audited A1 and compared screenshots.

\`\`\`qa-findings-json
{
  "summary": "One P1 scroll regression detected.",
  "slicesAudited": ["A1"],
  "findings": [
    {
      "title": "General pane cannot scroll",
      "severity": "P1",
      "slice": "A1",
      "category": "interaction-or-scroll",
      "fingerprint": "general-scroll-locked",
      "workaround": "Resize the window"
    }
  ]
}
\`\`\`
`
  const parsed = parseAuditReportFromText(tagged)
  assert.equal(parsed.findings.length, 1)
  assert.equal(parsed.findings[0]?.fingerprint, 'a1:general-scroll-locked')

  const fallback = `Report:\n\`\`\`json\n{"summary":"Clean","slicesAudited":["A1"],"findings":[]}\n\`\`\``
  assert.equal(parseAuditReportFromText(fallback).findings.length, 0)

  assert.throws(() => parseAuditReportFromText('No JSON block emitted here.'))
})

test('change-detector path matching, PR extraction, and slice fanout', () => {
  assert.equal(isRelevantMigrationPath('rust/crates/mt-pref-general/src/lib.rs'), true)
  assert.equal(isRelevantMigrationPath('packages/desktop/src/shared/flags/catalog.ts'), true)
  assert.equal(isRelevantMigrationPath('README.md'), false)
  assert.equal(isRelevantMigrationPath('.github/workflows/ci.yml'), false)

  assert.equal(extractPrNumberFromSubject('Merge pull request #5 from shashu10/slice-a1'), 5)
  assert.equal(extractPrNumberFromSubject('feat(a1): port general pane (#12)'), 12)
  assert.equal(extractPrNumberFromSubject('chore: tweak readme'), null)

  const activeIds = parseStatusActiveSliceIds(`
| \`P0.1\` | Feature flags | \`flags/*\` | — | \`packages/desktop/src/shared/flags/\` | \`verified\` |
| \`A1\` | General | \`prefs.pane.general\` | \`P0.3\` | \`prefComponents/general/\` | \`rust/crates/mt-pref-general\` | \`in-review\` |
| \`A2\` | Appearance | \`prefs.pane.appearance\` | \`P0.3\` | \`prefComponents/theme/\` | \`rust/crates/mt-pref-appearance\` | \`todo\` |
`)
  assert.deepEqual([...activeIds], ['A1'])

  const activeOnlyA1 = [SLICE_A1]
  // Shared host change fans out to all active slices (A1 only, not todo A2)
  assert.deepEqual(
    matchSlicesForFiles(['rust/crates/marktext-ui-kit/src/button.rs'], SAMPLE_SLICES, activeOnlyA1).map((s) => s.id),
    ['A1']
  )
  // Todo slice legacy change does not trigger an unmigrated slice
  assert.deepEqual(
    matchSlicesForFiles(['packages/desktop/src/renderer/src/prefComponents/theme/index.vue'], SAMPLE_SLICES, activeOnlyA1),
    []
  )
})

test('issue-filer formats title/body/labels and deduplicates by fingerprint', () => {
  const report: QaAuditReport = validateAuditReport({
    summary: 'Two findings, one already open.',
    slicesAudited: ['A1'],
    findings: [
      {
        title: 'Cannot scroll General pane',
        severity: 'P1',
        slice: 'A1',
        category: 'interaction-or-scroll',
        fingerprint: 'scroll-locked',
        summary: 'Vertical scrolling does not move content in General pane.',
        expected: 'Scrollable container reveals all lower settings.',
        actual: 'Content clips below viewport.',
        reproSteps: ['Open Settings -> General', 'Scroll down with mouse wheel'],
        workaround: 'Resize window vertically',
        rubricJustification: 'High severity with resize workaround -> P1.',
        evidence: ['After screenshot shows clipped bottom section'],
        relatedPrs: [5]
      },
      {
        title: 'Button border radius differs slightly',
        severity: 'P3',
        slice: 'A1',
        category: 'visual-polish',
        fingerprint: 'button-radius-delta',
        summary: 'Buttons render 2px radius instead of 4px.',
        expected: '4px border-radius',
        actual: '2px border-radius',
        reproSteps: ['Open Settings -> General'],
        workaround: null,
        rubricJustification: 'Pure cosmetic difference -> P3.',
        evidence: ['Visual diff on General pane buttons'],
        relatedPrs: [5]
      }
    ]
  })

  const meta = {
    repoSlug: 'shashu10/marktext',
    baseSha: '1111111111111111',
    headSha: '2222222222222222',
    agentId: 'bc-test-123',
    runId: 'run-456',
    runtime: 'cloud' as const,
    dryRun: true
  }

  const firstFinding = report.findings[0]
  assert.ok(firstFinding)
  assert.equal(renderIssueTitle(firstFinding), '[P1][A1] Cannot scroll General pane')
  assert.deepEqual(labelsForFinding(firstFinding), ['severity:P1', 'regression', 'qa-triage', 'slice:A1'])

  const body = renderIssueBody(firstFinding, meta)
  assert.equal(extractFingerprintFromBody(body), 'a1:scroll-locked')
  assert.match(body, /Resize window vertically/)
  assert.match(body, /#5/)

  const existing: ExistingRegressionIssue[] = [
    {
      number: 42,
      title: '[P1][A1] Cannot scroll General pane',
      url: 'https://github.com/shashu10/marktext/issues/42',
      fingerprint: 'a1:scroll-locked'
    }
  ]

  const records = fileAuditFindings({
    repoRoot: process.cwd(),
    report,
    existingIssues: existing,
    meta
  })

  assert.equal(records.length, 2)
  assert.equal(records[0]?.status, 'skipped-duplicate')
  assert.equal(records[0]?.duplicateOf, 42)
  assert.equal(records[1]?.status, 'dry-run')
})

test('prompt builder enforces non-fixing QA guardrails and includes rubric', () => {
  const prompt = buildQaAuditPrompt({
    repoSlug: 'shashu10/marktext',
    context: {
      baseSha: 'aaaaaaa',
      headSha: 'bbbbbbb',
      hasRelevantChanges: true,
      skipReason: null,
      commits: [
        {
          sha: 'bbbbbbb',
          shortSha: 'bbbbbbb',
          subject: 'feat(a1): port general pane (#5)',
          author: 'shashu10',
          date: '2026-11-22T00:00:00Z',
          prNumber: 5
        }
      ],
      changedFiles: ['rust/crates/mt-pref-general/src/lib.rs'],
      targetSlices: [SLICE_A1],
      allActiveSlices: [SLICE_A1]
    },
    openIssues: []
  })

  assert.match(prompt, /DO NOT FIX ANY BUGS/)
  assert.match(prompt, /severity:P0/)
  assert.match(prompt, /Cannot scroll|cannot scroll/i)
  assert.match(prompt, /Dropdown component changed into a list of buttons/)
  assert.match(prompt, /Button border-radius/)
  assert.match(prompt, /qa-findings-json/)
})

test('parseCliArgs and buildAgentOptions configure cloud vs local SDK options safely', () => {
  const repoRoot = resolve('.')
  const cloudOpts = parseCliArgs(['--runtime', 'cloud', '--slice', 'A1', '--dry-run'], repoRoot)
  assert.equal(cloudOpts.runtime, 'cloud')
  assert.equal(cloudOpts.slice, 'A1')
  assert.equal(cloudOpts.dryRun, true)

  const sdkCloud = buildAgentOptions({
    options: cloudOpts,
    repoRoot,
    apiKey: 'test-key',
    context: {
      baseSha: '11111111',
      headSha: '22222222',
      hasRelevantChanges: true,
      skipReason: null,
      commits: [],
      changedFiles: [],
      targetSlices: [SLICE_A1],
      allActiveSlices: [SLICE_A1]
    }
  })
  assert.equal(sdkCloud.cloud?.autoCreatePR, false)
  assert.equal(sdkCloud.cloud?.skipReviewerRequest, true)
  assert.equal(sdkCloud.cloud?.repos?.[0]?.startingRef, '22222222')
  assert.equal(sdkCloud.disallowedTools, undefined)

  const localOpts = parseCliArgs(['--runtime', 'local'], repoRoot)
  const sdkLocal = buildAgentOptions({
    options: localOpts,
    repoRoot,
    apiKey: 'test-key',
    context: {
      baseSha: null,
      headSha: '22222222',
      hasRelevantChanges: true,
      skipReason: null,
      commits: [],
      changedFiles: [],
      targetSlices: [SLICE_A1],
      allActiveSlices: [SLICE_A1]
    }
  })
  assert.equal(sdkLocal.cloud, undefined)
  assert.deepEqual(sdkLocal.disallowedTools, ['edit', 'delete'])
  assert.equal(sdkLocal.local?.cwd, repoRoot)
})
