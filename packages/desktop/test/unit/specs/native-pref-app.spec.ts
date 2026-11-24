import { describe, expect, it } from 'vitest'
import { mkdirSync, writeFileSync, rmSync } from 'node:fs'
import { join } from 'node:path'
import { tmpdir } from 'node:os'
import {
  applyUserKeybindingEdit,
  resolveDisplayedAccelerator,
  resolvePrefAppBinary
} from '../../../src/main/windows/nativeSetting'
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

  it('keeps an empty user accelerator as an unbind', () => {
    expect(resolveDisplayedAccelerator('', 'Ctrl+S')).toBe('')
    expect(resolveDisplayedAccelerator(undefined, 'Ctrl+S')).toBe('Ctrl+S')

    const defaults = new Map([['file.save', 'Ctrl+S']])
    const unbound = applyUserKeybindingEdit(new Map(), 'file.save', '', defaults)
    expect(unbound.get('file.save')).toBe('')

    const restored = applyUserKeybindingEdit(unbound, 'file.save', 'Ctrl+S', defaults)
    expect(restored.has('file.save')).toBe(false)
  })
})
