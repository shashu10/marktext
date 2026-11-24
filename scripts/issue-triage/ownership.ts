import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import type { SliceMetadata } from '../qa-monitor/change-detector.ts'
import type { LabelSpec } from '../qa-monitor/rubric.ts'

export interface OwnerTeamSpec {
  team: string
  area: string
  escalationChannel: string
  demoAssignee: string
}

export interface SharedPathOwnerRule extends OwnerTeamSpec {
  prefix: string
}

export interface OwnersConfig {
  demoMode: boolean
  demoAssignee: string
  demoModeNote: string
  surfaces: Record<string, OwnerTeamSpec>
  slices: Record<string, OwnerTeamSpec>
  sharedPaths: SharedPathOwnerRule[]
  fallback: OwnerTeamSpec
}

export interface ResolvedOwnerRoute {
  team: string
  area: string
  escalationChannel: string
  demoAssignee: string
  demoMode: boolean
  demoModeNote: string
  matchedBy: 'slice' | 'shared-path' | 'surface' | 'fallback'
  matchedKey: string
  matchedSlices: SliceMetadata[]
}

export const ELIGIBLE_TRIAGE_LABELS = new Set(['bug', 'regression'])
export const SKIP_TRIAGE_LABEL = 'triage:skip'
export const COMPLETE_TRIAGE_LABEL = 'triage:complete'

export function isTriageEligibleLabels(labels: string[]): {
  eligible: boolean
  reason: string
} {
  const normalized = labels.map((label) => label.trim().toLowerCase()).filter(Boolean)
  if (normalized.includes(SKIP_TRIAGE_LABEL)) {
    return { eligible: false, reason: `Issue carries "${SKIP_TRIAGE_LABEL}" opt-out label.` }
  }
  const matched = normalized.find((label) => ELIGIBLE_TRIAGE_LABELS.has(label))
  if (!matched) {
    return {
      eligible: false,
      reason: 'Issue does not have a "bug" or "regression" label.'
    }
  }
  return { eligible: true, reason: `Matched triage trigger label "${matched}".` }
}

export function loadOwnersConfig(repoRoot: string): OwnersConfig {
  const configPath = resolve(repoRoot, 'docs/migration/OWNERS.json')
  return JSON.parse(readFileSync(configPath, 'utf8')) as OwnersConfig
}

function normalizePath(filePath: string): string {
  return filePath.replace(/\\/g, '/').replace(/^\.\//, '')
}

function matchSliceById(sliceId: string, catalog: SliceMetadata[]): SliceMetadata | undefined {
  const needle = sliceId.trim().toLowerCase()
  return catalog.find((slice) => slice.id.toLowerCase() === needle)
}

function matchSlicesByFiles(files: string[], catalog: SliceMetadata[]): SliceMetadata[] {
  const matched = new Map<string, SliceMetadata>()
  for (const rawFile of files) {
    const file = normalizePath(rawFile)
    for (const slice of catalog) {
      const hitsCrate = file === slice.rustCrate || file.startsWith(`${slice.rustCrate}/`)
      const hitsLegacy = slice.legacyPaths.some(
        (legacy) => file === legacy || file.startsWith(`${legacy}/`)
      )
      if (hitsCrate || hitsLegacy) {
        matched.set(slice.id, slice)
      }
    }
  }
  return [...matched.values()]
}

export function resolveOwnerRoute(params: {
  affectedSliceIds: string[]
  suspectedFiles: string[]
  slicesCatalog: SliceMetadata[]
  owners: OwnersConfig
}): ResolvedOwnerRoute {
  const { affectedSliceIds, suspectedFiles, slicesCatalog, owners } = params

  const matchedById = affectedSliceIds
    .map((id) => matchSliceById(id, slicesCatalog))
    .filter((slice): slice is SliceMetadata => Boolean(slice))

  const matchedByFile = matchSlicesByFiles(suspectedFiles, slicesCatalog)
  const sliceMap = new Map<string, SliceMetadata>()
  for (const slice of [...matchedById, ...matchedByFile]) {
    sliceMap.set(slice.id, slice)
  }
  const matchedSlices = [...sliceMap.values()]

  // 1. Explicit slice override in OWNERS.json (highest specificity)
  for (const slice of matchedSlices) {
    const direct = Object.entries(owners.slices).find(
      ([key]) => key.toLowerCase() === slice.id.toLowerCase()
    )
    if (direct) {
      const [key, spec] = direct
      return {
        ...spec,
        demoMode: owners.demoMode,
        demoModeNote: owners.demoModeNote,
        matchedBy: 'slice',
        matchedKey: key,
        matchedSlices
      }
    }
  }

  // 2. Shared infrastructure path rule (e.g. wasm-host, ui-kit, flags)
  for (const rawFile of suspectedFiles) {
    const file = normalizePath(rawFile)
    const rule = owners.sharedPaths.find(
      (entry) => file === entry.prefix || file.startsWith(`${entry.prefix}/`)
    )
    if (rule) {
      return {
        team: rule.team,
        area: rule.area,
        escalationChannel: rule.escalationChannel,
        demoAssignee: rule.demoAssignee,
        demoMode: owners.demoMode,
        demoModeNote: owners.demoModeNote,
        matchedBy: 'shared-path',
        matchedKey: rule.prefix,
        matchedSlices
      }
    }
  }

  // 3. Surface ownership from matched slice(s)
  for (const slice of matchedSlices) {
    const surfaceOwner = owners.surfaces[slice.surface]
    if (surfaceOwner) {
      return {
        ...surfaceOwner,
        demoMode: owners.demoMode,
        demoModeNote: owners.demoModeNote,
        matchedBy: 'surface',
        matchedKey: slice.surface,
        matchedSlices
      }
    }
  }

  // 4. Fallback platform triage team
  return {
    ...owners.fallback,
    demoMode: owners.demoMode,
    demoModeNote: owners.demoModeNote,
    matchedBy: 'fallback',
    matchedKey: 'default',
    matchedSlices
  }
}

function truncateDescription(text: string): string {
  return text.length <= 100 ? text : `${text.slice(0, 97)}...`
}

export function buildRoutingLabelSpecs(route: ResolvedOwnerRoute): LabelSpec[] {
  const labels: LabelSpec[] = [
    {
      name: COMPLETE_TRIAGE_LABEL,
      color: '0e8a16',
      description: 'Automated root-cause triage and owner routing completed'
    },
    {
      name: route.team,
      color: '5319e7',
      description: truncateDescription(`Routed owner: ${route.area}`)
    }
  ]

  for (const slice of route.matchedSlices) {
    labels.push({
      name: `slice:${slice.id}`,
      color: '1d76db',
      description: truncateDescription(`Migration slice ${slice.id}`)
    })
  }

  return labels
}
