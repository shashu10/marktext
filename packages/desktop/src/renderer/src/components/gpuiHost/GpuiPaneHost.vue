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
import { usePreferencesStore } from '@/store/preferences'
import { useEditorStore } from '@/store/editor'
import { useKeybindingsStore } from '@/store/keybindings'
import { isMac } from '@/util'
import bus from '@/bus'
import { SpellChecker } from '@/spellchecker'
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
    const result = await window.electron.ipcRenderer.invoke('mt::preferences::update', { [key]: value }) as { status?: string }
    if (result.status === 'persisted') status = 'persisted'
  } catch {
    status = 'failed'
  }
  post({ type: 'PREFERENCE_RESULT', id, status })
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
  if (props.pane !== 'keybindings') return
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
}

onMounted(() => {
  window.addEventListener('message', onWindowMessage)
})

onBeforeUnmount(() => {
  window.removeEventListener('message', onWindowMessage)
})

watch(() => props.pane, (pane) => {
  frame.value?.removeAttribute('data-pane-ready')
  post({ type: 'SET_PANE', pane })
  void sendSpellcheckerInfo()
  void sendKeybindings()
})

watch(() => preferences.$state, () => {
  if (!model.value.showFrame) return
  sendSnapshot()
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
