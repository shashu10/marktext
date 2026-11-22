import environments from './environments.json'
import {
  type FlagKey,
  type FlagMode,
  type MigrationEnvironment,
  isFlagKey,
  isFlagMode,
  isMigrationEnvironment
} from './catalog'

export {
  FLAG_KEYS,
  FLAG_MODES,
  MIGRATION_ENVIRONMENTS,
  isFlagKey,
  isFlagMode,
  isMigrationEnvironment
} from './catalog'
export type { FlagKey, FlagMode, MigrationEnvironment } from './catalog'

export interface EnvironmentsFile {
  dev: Record<string, string>
  staging: Record<string, string>
  prod: Record<string, string>
}

export interface FlagResolutionInput {
  environment?: MigrationEnvironment
  overrides?: Record<string, string>
  environments?: EnvironmentsFile
  /** Vite dev server. Used only when `environment` is omitted. */
  devBuild?: boolean
  overrideEnvironment?: string | null
  overridesRaw?: string | null
}

const bundled = environments as EnvironmentsFile

/**
 * `MARKTEXT_MIGRATION_OVERRIDES` is a comma-separated `flag=mode` list.
 * The measurement harness uses it to force one slice without editing JSON.
 */
export const parseFlagOverrides = (raw: string | null | undefined): Record<string, FlagMode> => {
  const overrides: Record<string, FlagMode> = {}
  if (!raw) return overrides
  for (const part of raw.split(',')) {
    const trimmed = part.trim()
    if (!trimmed) continue
    const separator = trimmed.indexOf('=')
    if (separator <= 0) continue
    const key = trimmed.slice(0, separator).trim()
    const mode = trimmed.slice(separator + 1).trim()
    if (isFlagKey(key) && isFlagMode(mode)) overrides[key] = mode
  }
  return overrides
}

export const resolveMigrationEnvironment = (input: FlagResolutionInput = {}): MigrationEnvironment => {
  if (input.environment) return input.environment
  const override = input.overrideEnvironment?.trim()
  if (override && isMigrationEnvironment(override)) return override
  return input.devBuild ? 'dev' : 'prod'
}

export const resolveFlag = (key: string, input: FlagResolutionInput = {}): FlagMode => {
  const environment = resolveMigrationEnvironment(input)
  const table = (input.environments ?? bundled)[environment]
  const overrides = input.overrides ?? parseFlagOverrides(input.overridesRaw)
  const override = overrides[key]
  if (override && isFlagMode(override)) return override
  const configured = table[key]
  if (configured && isFlagMode(configured)) return configured
  return 'legacy'
}

export const flagKeyForPreferenceCategory = (category: string): FlagKey | null => {
  const key = `prefs.pane.${category}`
  return isFlagKey(key) ? key : null
}
