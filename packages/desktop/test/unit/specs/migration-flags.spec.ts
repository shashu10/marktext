import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'
import {
  FLAG_KEYS,
  resolveFlag,
  resolveMigrationEnvironment,
  parseFlagOverrides,
  flagKeyForPreferenceCategory
} from '@shared/flags'
import { frameMessageToCommand, paneHostModel } from '@/components/gpuiHost/protocol'

interface SliceManifest {
  slices: Array<{ flag: string }>
}

const manifest = JSON.parse(readFileSync(
  resolve(__dirname, '../../../../../docs/migration/slices.json'),
  'utf8'
)) as SliceManifest

describe('migration feature flags', () => {
  it('covers every slice flag and keeps each environment on legacy', () => {
    const sliceFlags = manifest.slices.map(slice => slice.flag)
    expect([...FLAG_KEYS].sort()).toEqual([...new Set(sliceFlags)].sort())
    for (const key of FLAG_KEYS) {
      expect(resolveFlag(key, { environment: 'prod' })).toBe('legacy')
      expect(resolveFlag(key, { environment: 'dev' })).toBe('legacy')
      expect(resolveFlag(key, { environment: 'staging' })).toBe('legacy')
    }
  })

  it('lets an override force one slice without changing the others', () => {
    const overrides = parseFlagOverrides('prefs.pane.general=rust-wasm, prefs.sidebar=shadow')
    expect(resolveFlag('prefs.pane.general', { environment: 'prod', overrides })).toBe('rust-wasm')
    expect(resolveFlag('prefs.sidebar', { environment: 'prod', overrides })).toBe('shadow')
    expect(resolveFlag('prefs.pane.editor', { environment: 'prod', overrides })).toBe('legacy')
  })

  it('uses the dev map for unpackaged dev builds unless the env overrides it', () => {
    expect(resolveMigrationEnvironment({ devBuild: true })).toBe('dev')
    expect(resolveMigrationEnvironment({ devBuild: true, overrideEnvironment: 'prod' })).toBe('prod')
    expect(resolveMigrationEnvironment({})).toBe('prod')
  })

  it('maps preference categories onto pane flags', () => {
    expect(flagKeyForPreferenceCategory('general')).toBe('prefs.pane.general')
    expect(flagKeyForPreferenceCategory('not-a-pane')).toBeNull()
  })
})

describe('GpuiPaneHost protocol', () => {
  it('keeps the Vue pane when the flag is legacy and swaps it for wasm', () => {
    expect(paneHostModel('prefs.pane.general', { environment: 'prod' })).toMatchObject({
      mode: 'legacy',
      showLegacy: true,
      showFrame: false
    })
    expect(paneHostModel('prefs.pane.general', {
      environment: 'prod',
      overrides: { 'prefs.pane.general': 'rust-wasm' }
    })).toMatchObject({
      showLegacy: false,
      showFrame: true
    })
    expect(paneHostModel('prefs.pane.general', {
      environment: 'prod',
      overrides: { 'prefs.pane.general': 'shadow' }
    })).toMatchObject({
      showLegacy: true,
      showFrame: true
    })
    expect(paneHostModel('window.preferences', {
      environment: 'prod',
      overrides: { 'window.preferences': 'gpui-native' }
    }).presentation).toBe('native')
  })

  it('turns guest messages into host commands', () => {
    expect(frameMessageToCommand({ type: 'SET_PREFERENCE', key: 'autoSave', value: true })).toEqual({
      type: 'set-preference',
      key: 'autoSave',
      value: true
    })
    expect(frameMessageToCommand({ type: 'SCROLL_TO_HEADER', slug: 'intro' })).toEqual({
      type: 'scroll-to-header',
      slug: 'intro'
    })
    expect(frameMessageToCommand({ type: 'TOGGLE_READING_MODE', readOnly: true })).toEqual({
      type: 'toggle-reading-mode',
      readOnly: true
    })
    expect(frameMessageToCommand({ type: 'READY', pane: 'general' })).toBeNull()
    expect(frameMessageToCommand(null)).toBeNull()
  })
})
