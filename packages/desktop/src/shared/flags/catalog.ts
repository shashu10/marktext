export const FLAG_MODES = ['legacy', 'rust-wasm', 'gpui-native', 'shadow'] as const

export type FlagMode = (typeof FLAG_MODES)[number]

export type MigrationEnvironment = 'dev' | 'staging' | 'prod'

export const MIGRATION_ENVIRONMENTS = ['dev', 'staging', 'prod'] as const

/**
 * Every slice flag from docs/migration/slices.json. Pane agents flip their own
 * key in environments.json; they do not invent new names.
 */
export const FLAG_KEYS = [
  'prefs.pane.general',
  'prefs.pane.appearance',
  'prefs.pane.editor',
  'prefs.pane.files',
  'prefs.pane.markdown',
  'prefs.pane.images',
  'prefs.pane.keybindings',
  'prefs.sidebar',
  'window.preferences',
  'dialog.about',
  'panel.docStats',
  'panel.outlineToc',
  'dialog.renameImport',
  'dialog.exportSettings',
  'dialog.commandPalette',
  'chrome.titleBar',
  'chrome.tabBar',
  'sidebar.files',
  'sidebar.search',
  'panel.findReplace',
  'chrome.misc',
  'core.services',
  'editor.sourceCode',
  'editor.muya.parser',
  'editor.muya.layout',
  'editor.muya.rich',
  'editor.muya.polish',
  'window.main'
] as const

export type FlagKey = (typeof FLAG_KEYS)[number]

export const isFlagMode = (value: string): value is FlagMode =>
  (FLAG_MODES as readonly string[]).includes(value)

export const isFlagKey = (value: string): value is FlagKey =>
  (FLAG_KEYS as readonly string[]).includes(value)

export const isMigrationEnvironment = (value: string): value is MigrationEnvironment =>
  (MIGRATION_ENVIRONMENTS as readonly string[]).includes(value)
