<template>
  <slot v-if="model.showLegacy && !model.showFrame" />
  <div
    v-else
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
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { usePreferencesStore } from '@/store/preferences'
import { useEditorStore } from '@/store/editor'
import bus from '@/bus'
import { frameMessageToCommand, paneHostModel } from './protocol'

const props = defineProps<{
  flagKey: string
  pane: string
}>()

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

const sendSnapshot = (): void => {
  post({
    type: 'PREFERENCES',
    snapshot: JSON.parse(JSON.stringify(preferences.$state)) as Record<string, unknown>
  })
}

const onWindowMessage = (event: MessageEvent): void => {
  if (event.source !== frame.value?.contentWindow) return
  const command = frameMessageToCommand(event.data)
  if (!command) return
  if (command.type === 'set-preference') {
    window.electron.ipcRenderer.send('mt::set-user-preference', { [command.key]: command.value })
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
  height: 100%;
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
