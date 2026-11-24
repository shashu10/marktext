import { describe, expect, it } from 'vitest'
import { mkdirSync, writeFileSync, rmSync } from 'node:fs'
import { join } from 'node:path'
import { tmpdir } from 'node:os'
import { resolvePrefAppBinary } from '../../../src/main/windows/nativeSetting'
import { resolveFlag } from '@shared/flags'

describe('native preferences launcher', () => {
  it('resolves window.preferences=gpui-native from MARKTEXT_MIGRATION_OVERRIDES', () => {
    expect(
      resolveFlag('window.preferences', {
        environment: 'prod',
        overridesRaw: 'window.preferences=gpui-native'
      })
    ).toBe('gpui-native')
    expect(resolveFlag('window.preferences', { environment: 'prod' })).toBe('legacy')
  })

  it('prefers MARKTEXT_PREF_APP when the file exists', () => {
    const dir = join(tmpdir(), `mt-pref-bin-${process.pid}`)
    mkdirSync(dir, { recursive: true })
    const binary = join(dir, 'mt-pref-app')
    writeFileSync(binary, '#!/bin/true\n', { mode: 0o755 })
    try {
      expect(resolvePrefAppBinary({ MARKTEXT_PREF_APP: binary }, dir)).toBe(binary)
      expect(resolvePrefAppBinary({ MARKTEXT_PREF_APP: join(dir, 'missing') }, dir)).toBeNull()
    } finally {
      rmSync(dir, { recursive: true, force: true })
    }
  })
})
