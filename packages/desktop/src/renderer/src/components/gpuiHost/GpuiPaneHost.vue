<template>
  <slot v-if="model.showLegacy && !model.showFrame && !hasFallthrough" />
  <div
    v-else-if="model.showLegacy && !model.showFrame"
    v-bind="attrs"
  >
    <slot />
  </div>
  <div
    v-else
    v-bind="attrs"
    class="gpui-pane-host"
    :class="{ 'is-shadow': model.mode === 'shadow', 'is-native': model.presentation === 'native' }"
    :data-flag="flagKey"
    :data-mode="model.mode"
    :data-pane="pane"
  >
    <slot v-if="model.showLegacy" />
    <iframe
      v-if="model.showFrame"
      ref="frame"
      class="gpui-pane-frame"
      title="MarkText GPUI pane"
      src="gpui-host/index.html"
      @load="hello"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, useAttrs, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { usePreferencesStore } from '@/store/preferences'
import { useEditorStore } from '@/store/editor'
import { useKeybindingsStore } from '@/store/keybindings'
import { isMac } from '@/util'
import bus from '@/bus'
import { SpellChecker } from '@/spellchecker'
import { openSetting } from '@/prefComponents/settingLocation'
import { getCategory } from '@/prefComponents/sideBar/config'
import { frameMessageToCommand, isGuestReady, paneChangedName, paneHostModel } from './protocol'

// router-view's class is the preferences scroll container. A bare slot drops
// it, so fallthrough attrs are placed on an element when the parent sent any.
defineOptions({ inheritAttrs: false })

const props = defineProps<{
  flagKey: string
  pane: string
}>()

const attrs = useAttrs()
const hasFallthrough = computed(() => Object.keys(attrs).length > 0)

const frame = ref<HTMLIFrameElement | null>(null)
const preferences = usePreferencesStore()
const keybindings = useKeybindingsStore()
const route = useRoute()
const router = useRouter()

const migrationInput = (): { devBuild: boolean; overrideEnvironment?: string; overridesRaw?: string } => {
  const env = window.process?.env
  return {
    devBuild: import.meta.env.DEV,
    overrideEnvironment: env?.MARKTEXT_MIGRATION_ENV,
    overridesRaw: env?.MARKTEXT_MIGRATION_OVERRIDES
  }
}

const model = computed(() => paneHostModel(props.flagKey, migrationInput()))

const post = (message: Record<string, unknown>): void => {
  frame.value?.contentWindow?.postMessage(message, '*')
}

const persistPreference = async (id: number, key: string, value: unknown): Promise<void> => {
  let status: 'persisted' | 'failed' = 'failed'
  try {
    // An empty key carries a multi-key patch object (startup folder selection).
    const patch = key === '' && value !== null && typeof value === 'object' && !Array.isArray(value)
      ? value as Record<string, unknown>
      : { [key]: value }
    const result = await window.electron.ipcRenderer.invoke('mt::preferences::update', patch) as { status?: string }
    if (result.status === 'persisted') status = 'persisted'
  } catch {
    status = 'failed'
  }
  post({ type: 'PREFERENCE_RESULT', id, status })
}


const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null

const runHostAction = async (id: number, action: string, payload: unknown): Promise<void> => {
  const body = isRecord(payload) ? payload : {}
  let status = 'failed'
  let result: Record<string, unknown> = {}
  try {
    if (action === 'set-user-data' && typeof body.key === 'string') {
      window.electron.ipcRenderer.send('mt::set-user-data', { [body.key]: body.value })
      status = 'ok'
      result = { [body.key]: body.value }
    } else if (action === 'ask-image-folder') {
      const path = typeof body.path === 'string' ? body.path : undefined
      window.electron.ipcRenderer.send('mt::ask-for-modify-image-folder-path', path)
      status = 'ok'
      if (typeof path === 'string') result = { imageFolderPath: path }
    } else if (action === 'open-path' && typeof body.path === 'string') {
      await window.electron.shell.openPath(body.path)
      status = 'ok'
    } else if (action === 'open-external' && typeof body.url === 'string') {
      await window.electron.shell.openExternal(body.url)
      status = 'ok'
    } else if (action === 'probe-pandoc') {
      const info = await window.electron.ipcRenderer.invoke('mt::pandoc::command') as { command?: string | null }
      status = 'ok'
      result = { command: info?.command ?? '' }
    } else if (action === 'probe-picgo') {
      const detectedAt = new Date().toISOString()
      const debug: string[] = [`Detection time: ${detectedAt}`, `Platform: ${window.process?.platform || 'unknown'}`]
      if (!window.commandExists || typeof window.commandExists.exists !== 'function') {
        throw new Error('Command detection bridge is unavailable')
      }
      const [nodeExists, npmExists, exists] = await Promise.all([
        window.commandExists.exists('node'),
        window.commandExists.exists('npm'),
        window.commandExists.exists('picgo')
      ])
      debug.push(`Node.js detected: ${nodeExists}`, `npm detected: ${npmExists}`, `PicGo-Core detected: ${exists}`)
      status = 'ok'
      result = {
        exists,
        detectedAt,
        debug: debug.join('\n'),
        ...(exists ? { successAt: detectedAt } : {})
      }
    } else if (action === 'is-executable' && typeof body.path === 'string') {
      const executable = await window.fileUtils.isExecutable(body.path)
      status = 'ok'
      result = { path: body.path, executable }
    } else if (action === 'navigate-preference' && typeof body.category === 'string') {
      const category = body.category
      const settingKey = typeof body.key === 'string' ? body.key : undefined
      if (settingKey) {
        await openSetting(router, category, settingKey)
      } else {
        await router.push({ path: `/preference/${category}` })
      }
      status = 'ok'
      result = { category, ...(settingKey ? { key: settingKey } : {}) }
    }
  } catch (error) {
    status = 'failed'
    result = {
      error: error instanceof Error ? (error.stack || error.message) : String(error)
    }
  }
  post({ type: 'HOST_ACTION_RESULT', id, status, payload: result })
}

const chooseDirectory = async (id: number): Promise<void> => {
  let path: string | null
  try {
    path = await window.electron.ipcRenderer.invoke('mt::preferences::choose-directory') as string | null
  } catch {
    path = null
  }
  post({ type: 'DIRECTORY_CHOSEN', id, path })
}

const removeSpellcheckerWord = async (id: number, word: string): Promise<void> => {
  let ok = false
  try {
    ok = Boolean(await window.electron.ipcRenderer.invoke('mt::spellchecker-remove-word', word))
  } catch {
    // leave ok false
  }
  post({ type: 'SPELLCHECKER_REMOVE_RESULT', id, ok })
}

const sendSpellcheckerInfo = async (): Promise<void> => {
  if (props.pane !== 'editor') return
  let dictionaries: string[] = []
  let words: string[] = []
  if (!isMac) {
    try {
      dictionaries = await SpellChecker.getAvailableDictionaries()
    } catch {
      dictionaries = ['en-US']
    }
    try {
      words = await window.electron.ipcRenderer.invoke('mt::spellchecker-get-custom-dictionary-words')
    } catch {
      words = []
    }
  }
  post({
    type: 'SPELLCHECKER_INFO',
    isOsx: isMac,
    dictionaries,
    words
  })
}

const sendSnapshot = (): void => {
  post({
    type: 'PREFERENCES',
    snapshot: JSON.parse(JSON.stringify(preferences.$state)) as Record<string, unknown>
  })
  void sendSpellcheckerInfo()
}

const keybindingCommandsPayload = () => keybindings.commands.map(command => ({
  id: command.id,
  description: command.description,
  accelerator: command.accelerator,
  defaultAccelerator: command.defaultAccelerator
}))

const sendKeybindings = async (): Promise<void> => {
  if (props.pane !== 'keybindings' && props.pane !== 'sidebar') return
  post({
    type: 'KEYBINDINGS',
    commands: [],
    isMac,
    showDebug: Boolean(window.marktext?.env?.debug),
    loading: true,
    loadError: false
  })
  await keybindings.ensureLoaded()
  post({
    type: 'KEYBINDINGS',
    commands: keybindingCommandsPayload(),
    isMac,
    showDebug: Boolean(window.marktext?.env?.debug),
    loading: keybindings.loading,
    loadError: keybindings.loadError
  })
}

const sendActiveCategory = (): void => {
  if (props.pane !== 'sidebar') return
  const category = typeof route.name === 'string' ? route.name : 'general'
  post({ type: 'ACTIVE_CATEGORY', category })
}

const saveKeybinding = async (id: number, commandId: string, accelerator: string): Promise<void> => {
  await keybindings.change(commandId, accelerator)
  if (keybindings.conflict) {
    post({
      type: 'KEYBINDING_RESULT',
      id,
      status: 'conflict',
      conflictAccelerator: keybindings.conflict.accelerator,
      conflictCommandIds: keybindings.conflict.commands.map(command => command.id),
      commands: keybindingCommandsPayload()
    })
    return
  }
  const entry = keybindings.commands.find(command => command.id === commandId)
  const applied = entry != null && (
    accelerator === ''
      ? entry.accelerator === ''
      : entry.accelerator === accelerator || entry.accelerator === entry.defaultAccelerator
  )
  post({
    type: 'KEYBINDING_RESULT',
    id,
    status: applied ? 'persisted' : 'failed',
    commands: keybindingCommandsPayload()
  })
}

const resetAllKeybindings = async (id: number): Promise<void> => {
  await keybindings.resetAll()
  post({
    type: 'KEYBINDING_RESULT',
    id,
    status: keybindings.hasCustomBindings ? 'failed' : 'persisted',
    commands: keybindingCommandsPayload()
  })
}

const onWindowMessage = (event: MessageEvent): void => {
  if (event.source !== frame.value?.contentWindow) return
  // The measure harness waits for this attribute and a sized canvas. The
  // settings document's DOMContentLoaded fires before either exists.
  const readyPane = paneChangedName(event.data)
  if (readyPane) frame.value?.setAttribute('data-pane-ready', readyPane)
  // iframe `load` fires before wasm finishes installing its listener, so the
  // first SET_PANE is lost. READY means the guest can accept the pane now.
  if (isGuestReady(event.data)) {
    post({ type: 'SET_PANE', pane: props.pane })
    sendSnapshot()
    void sendKeybindings()
    sendActiveCategory()
  }
  const command = frameMessageToCommand(event.data)
  if (!command) return
  if (command.type === 'set-preference') {
    window.electron.ipcRenderer.send('mt::set-user-preference', { [command.key]: command.value })
    if (command.key === 'spellcheckerLanguage' && typeof command.value === 'string') {
      void window.electron.ipcRenderer.invoke('mt::spellchecker-switch-language', command.value)
    }
    return
  }
  if (command.type === 'persist-preference') {
    void persistPreference(command.id, command.key, command.value)
    return
  }
  if (command.type === 'host-action') {
    void runHostAction(command.id, command.action, command.payload)
    return
  }
  if (command.type === 'choose-directory') {
    void chooseDirectory(command.id)
    return
  }
  if (command.type === 'spellchecker-remove-word') {
    void removeSpellcheckerWord(command.id, command.word)
    return
  }
  if (command.type === 'scroll-to-header') {
    bus.emit('scroll-to-header', command.slug)
    return
  }
  if (command.type === 'request-keybindings') {
    void sendKeybindings()
    return
  }
  if (command.type === 'save-keybinding') {
    void saveKeybinding(command.id, command.commandId, command.accelerator)
    return
  }
  if (command.type === 'reset-all-keybindings') {
    void resetAllKeybindings(command.id)
    return
  }
  if (command.type === 'keybinding-debug-dump') {
    window.electron.ipcRenderer.send('mt::keybinding-debug-dump-keyboard-info')
    return
  }
  const editor = useEditorStore()
  if (editor.currentFile) {
    void editor.SET_READ_ONLY(editor.currentFile.id, command.readOnly)
  }
  bus.emit('gpui:toggle-reading-mode', command.readOnly)
}

const hello = (): void => {
  frame.value?.removeAttribute('data-pane-ready')
  post({ type: 'HOST_HELLO' })
  post({ type: 'SET_PANE', pane: props.pane })
  sendSnapshot()
  void sendKeybindings()
  sendActiveCategory()
}

const onIpcCategoryChange = (_event: unknown, category: unknown): void => {
  if (props.pane !== 'sidebar' || typeof category !== 'string') return
  const target = getCategory().find(item => item.label === category)
  if (target) void router.push({ path: target.path })
}

let stopCategoryListener: (() => void) | null = null

onMounted(() => {
  window.addEventListener('message', onWindowMessage)
  if (props.pane === 'sidebar') {
    stopCategoryListener = window.electron.ipcRenderer.on('settings::change-tab', onIpcCategoryChange)
  }
})

onBeforeUnmount(() => {
  window.removeEventListener('message', onWindowMessage)
  stopCategoryListener?.()
  stopCategoryListener = null
})

watch(() => props.pane, (pane) => {
  frame.value?.removeAttribute('data-pane-ready')
  post({ type: 'SET_PANE', pane })
  void sendSpellcheckerInfo()
  void sendKeybindings()
  sendActiveCategory()
})

watch(() => route.name, () => {
  if (!model.value.showFrame) return
  sendActiveCategory()
})

watch(() => preferences.$state, () => {
  if (!model.value.showFrame) return
  sendSnapshot()
}, { deep: true })

watch(() => keybindings.commands, () => {
  if (!model.value.showFrame || props.pane !== 'sidebar') return
  post({
    type: 'KEYBINDINGS',
    commands: keybindingCommandsPayload(),
    isMac,
    showDebug: Boolean(window.marktext?.env?.debug),
    loading: keybindings.loading,
    loadError: keybindings.loadError
  })
}, { deep: true })
</script>

<style scoped>
.gpui-pane-host {
  display: flex;
  flex: 1;
  min-height: 0;
}
.gpui-pane-frame {
  flex: 1;
  width: 100%;
  height: 100%;
  border: 0;
  background: var(--editorBgColor);
}
.gpui-pane-host.is-shadow {
  position: relative;
}
.gpui-pane-host.is-shadow .gpui-pane-frame {
  position: absolute;
  width: 1px;
  height: 1px;
  opacity: 0;
  pointer-events: none;
}
</style>
