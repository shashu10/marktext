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
import bus from '@/bus'
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
    }
  } catch (error) {
    status = 'failed'
    result = {
      error: error instanceof Error ? (error.stack || error.message) : String(error)
    }
  }
  post({ type: 'HOST_ACTION_RESULT', id, status, payload: result })
}

const sendSnapshot = (): void => {
  post({
    type: 'PREFERENCES',
    snapshot: JSON.parse(JSON.stringify(preferences.$state)) as Record<string, unknown>
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
  }
  const command = frameMessageToCommand(event.data)
  if (!command) return
  if (command.type === 'set-preference') {
    window.electron.ipcRenderer.send('mt::set-user-preference', { [command.key]: command.value })
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
  if (command.type === 'scroll-to-header') {
    bus.emit('scroll-to-header', command.slug)
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
