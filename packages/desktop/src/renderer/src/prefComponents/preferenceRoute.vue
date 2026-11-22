<template>
  <gpui-pane-host
    :flag-key="flagKey"
    :pane="category"
  >
    <markdown-settings v-if="category === 'markdown'" />
    <keybindings-settings v-else-if="category === 'keybindings'" />
    <category-page
      v-else
      :category="category"
    />
  </gpui-pane-host>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import GpuiPaneHost from '@/components/gpuiHost/GpuiPaneHost.vue'
import CategoryPage from './categoryPage.vue'
import MarkdownSettings from './markdown/index.vue'
import KeybindingsSettings from './keybindings/index.vue'
import type { PreferenceCategoryId } from './categories'

const props = defineProps<{ category: PreferenceCategoryId }>()
const flagKey = computed(() => `prefs.pane.${props.category}`)
</script>
