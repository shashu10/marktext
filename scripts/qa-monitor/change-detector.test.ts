import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { mkdirSync, mkdtempSync, renameSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'
import { collectChangeAuditContext, isRelevantMigrationPath } from './change-detector.ts'

test('migration directories include nested files, while named files match exactly', () => {
  assert.equal(isRelevantMigrationPath('packages/muya/src/editor/doc.ts'), true)
  assert.equal(isRelevantMigrationPath('docs/migration/STATUS.md'), true)
  assert.equal(isRelevantMigrationPath('scripts/migration-measure.ts.bak'), false)
  assert.equal(isRelevantMigrationPath('packages/desktop/src-old/view.vue'), false)
})

test('the first audit covers every active slice and moving a file out still audits its old slice', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'qa-changed-paths-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const git = (...args: string[]) => execFileSync('git', [
    '-c', 'user.name=QA fixture', '-c', 'user.email=qa@example.invalid', '-c', 'commit.gpgSign=false',
    ...args
  ], { cwd: root, encoding: 'utf8' }).trim()

  git('init', '-q')
  const ids = ['A1', 'A2']
  mkdirSync(join(root, 'docs/migration'), { recursive: true })
  const slices = ids.map((id) => ({ id, rustCrate: `rust/${id}`, legacyPaths: [] }))
  writeFileSync(join(root, 'docs/migration/slices.json'), JSON.stringify({ slices }))
  writeFileSync(join(root, 'docs/migration/STATUS.md'), ids.map((id) => `| ${id} | verified |`).join('\n'))
  git('add', '.')
  git('commit', '-qm', 'Register both panes')
  mkdirSync(join(root, 'rust/A1'), { recursive: true })
  writeFileSync(join(root, 'rust/A1/view.rs'), 'fn view() {}')
  git('add', '.')
  git('commit', '-qm', 'Touch just one pane')
  const initialHead = git('rev-parse', 'HEAD')
  const options = { repoRoot: root, requestedSlice: 'auto', force: false }
  const baseline = collectChangeAuditContext({ ...options, baseSha: null, headSha: initialHead })
  assert.deepEqual(baseline.targetSlices.map((slice) => slice.id), ids)

  renameSync(join(root, 'rust/A1/view.rs'), join(root, 'old-view.txt'))
  git('add', '-A')
  git('commit', '-qm', 'Move file out of the audited pane')
  const headSha = git('rev-parse', 'HEAD')
  const delta = collectChangeAuditContext({ ...options, baseSha: initialHead, headSha })
  assert.deepEqual(delta.targetSlices.map((slice) => slice.id), ['A1'])
  assert.equal(delta.hasRelevantChanges, true)
  assert.throws(() => collectChangeAuditContext({ ...options, baseSha: 'missing-ref', headSha }))
})
