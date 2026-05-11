<script setup lang="ts">
import type { SettingsSection } from '../../utils/types/settings';

defineProps<{
  activeSection: SettingsSection;
}>();

defineEmits<{
  change: [section: SettingsSection];
}>();

const sections: Array<{ key: SettingsSection; label: string; description: string }> = [
  {
    key: 'devices',
    label: '设备设置',
    description: '输入、输出与虚拟麦克风',
  },
  {
    key: 'backends',
    label: '后端设置',
    description: 'LLM、FunSpeech 与 MCP',
  },
];
</script>

<template>
  <div class="settings-tabs" role="tablist" aria-label="设置分段">
    <button
      v-for="section in sections"
      :key="section.key"
      type="button"
      role="tab"
      :aria-selected="section.key === activeSection"
      :class="{ 'settings-tab--active': section.key === activeSection }"
      @click="$emit('change', section.key)"
    >
      <span>{{ section.label }}</span>
      <small>{{ section.description }}</small>
    </button>
  </div>
</template>
