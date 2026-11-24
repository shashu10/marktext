import assert from 'node:assert/strict'
import { resolve } from 'node:path'
import test from 'node:test'
import type { SliceMetadata } from '../qa-monitor/change-detector.ts'
import { findDriftedSlices } from './drift-detector.ts'
import { buildLegacySyncPrompt } from './prompt.ts'
import { buildSyncAgentOptions, parseLegacySyncCliArgs } from './run-legacy-sync.ts'

const REPO_ROOT = resolve('.')

const SLICE_A1: SliceMetadata = {
  id: 'A1',
  name: 'Preferences — General Pane',
  surface: 'preferences',
  flag: 'prefs.pane.general',
  rustCrate: 'rust/crates/mt-pref-general',
  legacyPaths: [
    'packages/desktop/src/renderer/src/prefComponents/general/index.vue',
    'packages/desktop/src/renderer/src/prefComponents/general/config.ts'
  ]
}

const SLICE_A5: SliceMetadata = {
  id: 'A5',
  name: 'Preferences — Markdown & Diagrams Pane',
  surface: 'preferences',
  flag: 'prefs.pane.markdown',
  rustCrate: 'rust/crates/mt-pref-markdown',
  legacyPaths: ['packages/desktop/src/renderer/src/prefComponents/markdown']
}

test('findDriftedSlices flags legacy-only edits on active slices and skips synced or unrelated edits', () => {
  const activeSlices = [SLICE_A1, SLICE_A5]

  // Legacy-only change on A1 -> drift detected
  const drifted = findDriftedSlices({
    changedFiles: ['packages/desktop/src/renderer/src/prefComponents/general/index.vue'],
    activeSlices
  })
  assert.equal(drifted.length, 1)
  assert.equal(drifted[0]?.slice.id, 'A1')

  // Both legacy and Rust crate updated together -> already synced, $0 skip
  const synced = findDriftedSlices({
    changedFiles: [
      'packages/desktop/src/renderer/src/prefComponents/general/index.vue',
      'rust/crates/mt-pref-general/src/lib.rs'
    ],
    activeSlices
  })
  assert.equal(synced.length, 0)

  // Force flag overrides same-commit Rust update check
  const forced = findDriftedSlices({
    changedFiles: [
      'packages/desktop/src/renderer/src/prefComponents/general/index.vue',
      'rust/crates/mt-pref-general/src/lib.rs'
    ],
    activeSlices,
    force: true
  })
  assert.equal(forced.length, 1)

  // Unrelated legacy path (unmigrated slice) -> no drift
  const unrelated = findDriftedSlices({
    changedFiles: ['packages/desktop/src/renderer/src/components/about/index.vue', 'README.md'],
    activeSlices
  })
  assert.equal(unrelated.length, 0)
})

test('buildLegacySyncPrompt includes diff, target crate, MEMORY.md, and prod flag guardrails', () => {
  const prompt = buildLegacySyncPrompt({
    repoSlug: 'shashu10/marktext',
    baseSha: '11111111aaaa',
    headSha: '22222222bbbb',
    drifted: {
      slice: SLICE_A5,
      changedLegacyFiles: ['packages/desktop/src/renderer/src/prefComponents/markdown/index.vue'],
      changedRustFiles: [],
      legacyDiff: '+const enableFootnotes = true'
    },
    commits: [
      {
        sha: '22222222bbbb',
        shortSha: '2222222',
        subject: 'feat(markdown): add footnotes toggle (#99)',
        author: 'shashu10',
        date: '2026-11-24T00:00:00Z',
        prNumber: 99
      }
    ]
  })

  assert.match(prompt, /rust\/crates\/mt-pref-markdown/)
  assert.match(prompt, /\+const enableFootnotes = true/)
  assert.match(prompt, /docs\/migration\/MEMORY\.md/)
  assert.match(prompt, /DO NOT flip `prod` defaults/)
  assert.match(prompt, /cargo test --manifest-path rust\/Cargo\.toml -p mt-pref-markdown/)
})

test('parseLegacySyncCliArgs and buildSyncAgentOptions configure cloud autoCreatePR safely', () => {
  const cloudOpts = parseLegacySyncCliArgs(
    ['--base-sha', '11111111', '--head-sha', '22222222', '--slice', 'A1'],
    REPO_ROOT
  )
  assert.equal(cloudOpts.runtime, 'cloud')
  assert.equal(cloudOpts.noPr, false)

  const drifted = {
    slice: SLICE_A1,
    changedLegacyFiles: ['packages/desktop/src/renderer/src/prefComponents/general/index.vue'],
    changedRustFiles: [],
    legacyDiff: ''
  }

  const sdkCloud = buildSyncAgentOptions({
    options: cloudOpts,
    repoRoot: REPO_ROOT,
    baseSha: '11111111aaaa',
    headSha: '22222222bbbb',
    drifted,
    apiKey: 'test-key'
  })
  assert.equal(sdkCloud.cloud?.autoCreatePR, true)
  assert.equal(sdkCloud.cloud?.skipReviewerRequest, true)
  assert.equal(sdkCloud.idempotencyKey, 'legacy-sync-1111111-2222222-A1')

  const noPrOpts = parseLegacySyncCliArgs(['--no-pr'], REPO_ROOT)
  const sdkNoPr = buildSyncAgentOptions({
    options: noPrOpts,
    repoRoot: REPO_ROOT,
    baseSha: '11111111aaaa',
    headSha: '22222222bbbb',
    drifted,
    apiKey: 'test-key'
  })
  assert.equal(sdkNoPr.cloud?.autoCreatePR, false)

  const localOpts = parseLegacySyncCliArgs(['--runtime', 'local'], REPO_ROOT)
  const sdkLocal = buildSyncAgentOptions({
    options: localOpts,
    repoRoot: REPO_ROOT,
    baseSha: '11111111aaaa',
    headSha: '22222222bbbb',
    drifted,
    apiKey: 'test-key'
  })
  assert.equal(sdkLocal.cloud, undefined)
  assert.equal(sdkLocal.local?.cwd, REPO_ROOT)
})
