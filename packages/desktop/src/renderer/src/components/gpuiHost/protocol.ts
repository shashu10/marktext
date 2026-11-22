import { resolveFlag, type FlagMode, type FlagResolutionInput } from '@shared/flags'

export type GuestMessage =
  | { type: 'READY'; pane: string | null }
  | { type: 'PANE_CHANGED'; pane: string }
  | { type: 'PREFERENCES_APPLIED' }
  | { type: 'SET_PREFERENCE'; key: string; value: unknown }
  | { type: 'SCROLL_TO_HEADER'; slug: string }
  | { type: 'TOGGLE_READING_MODE'; readOnly: boolean }

export type HostCommand =
  | { type: 'set-preference'; key: string; value: unknown }
  | { type: 'scroll-to-header'; slug: string }
  | { type: 'toggle-reading-mode'; readOnly: boolean }

export type PanePresentation = 'legacy' | 'wasm' | 'native'

export interface PaneHostModel {
  mode: FlagMode
  presentation: PanePresentation
  /** Shadow mode still paints the Vue pane. The iframe only observes. */
  showLegacy: boolean
  showFrame: boolean
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null

export const paneHostModel = (flagKey: string, input: FlagResolutionInput = {}): PaneHostModel => {
  const mode = resolveFlag(flagKey, input)
  if (mode === 'legacy') {
    return { mode, presentation: 'legacy', showLegacy: true, showFrame: false }
  }
  if (mode === 'shadow') {
    return { mode, presentation: 'wasm', showLegacy: true, showFrame: true }
  }
  if (mode === 'gpui-native') {
    return { mode, presentation: 'native', showLegacy: false, showFrame: false }
  }
  return { mode, presentation: 'wasm', showLegacy: false, showFrame: true }
}

export const isGuestReady = (message: unknown): boolean =>
  isRecord(message) && message.type === 'READY'

export const paneChangedName = (message: unknown): string | null => {
  if (!isRecord(message) || message.type !== 'PANE_CHANGED' || typeof message.pane !== 'string') return null
  if (!message.pane) return null
  return message.pane
}

export const frameMessageToCommand = (message: unknown): HostCommand | null => {
  if (!isRecord(message) || typeof message.type !== 'string') return null
  if (message.type === 'SET_PREFERENCE' && typeof message.key === 'string') {
    return { type: 'set-preference', key: message.key, value: message.value }
  }
  if (message.type === 'SCROLL_TO_HEADER' && typeof message.slug === 'string') {
    return { type: 'scroll-to-header', slug: message.slug }
  }
  if (message.type === 'TOGGLE_READING_MODE' && typeof message.readOnly === 'boolean') {
    return { type: 'toggle-reading-mode', readOnly: message.readOnly }
  }
  return null
}

export const isGuestMessage = (message: unknown): message is GuestMessage =>
  isRecord(message) && typeof message.type === 'string' && (
    message.type === 'READY' ||
    message.type === 'PANE_CHANGED' ||
    message.type === 'PREFERENCES_APPLIED' ||
    message.type === 'SET_PREFERENCE' ||
    message.type === 'SCROLL_TO_HEADER' ||
    message.type === 'TOGGLE_READING_MODE'
  )
