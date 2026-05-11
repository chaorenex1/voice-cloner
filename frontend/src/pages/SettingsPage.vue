<script setup lang="ts">
import { onMounted, watch } from 'vue';
import BackendSettingsForm from '../components/settings/BackendSettingsForm.vue';
import DeviceSettingsForm from '../components/settings/DeviceSettingsForm.vue';
import SettingsSectionTabs from '../components/settings/SettingsSectionTabs.vue';
import { useSettingsStore } from '../stores/settings.store';

const {
  state,
  loadSettings,
  setSection,
  updateDeviceSettings,
  updateBackendSettings,
  updateFunSpeechSettings,
  updateMcpSettings,
  updateRuntimeSettings,
  saveSettings,
} = useSettingsStore();

// const activeTitle = computed(() => (state.activeSection === 'devices' ? '设备设置' : '后端设置'));
const props = defineProps<{
  returnTarget?: string | null;
}>();

defineEmits<{
  back: [];
}>();

function ensureDeviceSectionForReturn(): void {
  if (props.returnTarget === 'realtime') {
    setSection('devices');
  }
}

onMounted(() => {
  ensureDeviceSectionForReturn();
  if (!state.settings) {
    void loadSettings();
  }
});

watch(() => props.returnTarget, ensureDeviceSectionForReturn);
</script>

<template>
  <section class="module-page settings-page">
    <header class="page-toolbar settings-toolbar">
      <div>
        <p class="module-eyebrow">Settings</p>
        <!-- <h2>{{ activeTitle }}</h2> -->
        <p class="module-description">管理音频设备、虚拟麦克风、FunSpeech、LLM 和 MCP 后端能力。</p>
      </div>
      <button
        v-if="returnTarget === 'realtime'"
        class="ghost-button"
        type="button"
        @click="$emit('back')"
      >
        返回实时通话
      </button>
    </header>

    <SettingsSectionTabs :active-section="state.activeSection" @change="setSection" />

    <div v-if="state.activeSection === 'devices'" class="settings-layout">
      <DeviceSettingsForm
        :devices="state.audioDevices"
        :settings="state.settings?.device ?? null"
        @update="updateDeviceSettings"
        @commit="saveSettings"
      />
      <section class="settings-card">
        <div class="settings-card__header">
          <p class="module-eyebrow">Realtime Debug</p>
          <span>控制实时诊断信息和 ACK 时机。默认入本地播放队列即确认。</span>
        </div>

        <div v-if="state.settings" class="settings-form">
          <label class="toggle-field">
            <input
              :checked="state.settings.runtime.realtimePlaybackAckEnabled"
              type="checkbox"
              @change="
                updateRuntimeSettings({
                  realtimePlaybackAckEnabled: ($event.target as HTMLInputElement).checked,
                })
              "
              @blur="saveSettings"
            />
            <span>ACK 等到成功播放/写入后再发送</span>
          </label>
          <label class="toggle-field">
            <input
              :checked="state.settings.runtime.realtimeDebugEnabled"
              type="checkbox"
              @change="
                updateRuntimeSettings({
                  realtimeDebugEnabled: ($event.target as HTMLInputElement).checked,
                })
              "
              @blur="saveSettings"
            />
            <span>启用实时调试信息</span>
          </label>
        </div>
      </section>
    </div>

    <BackendSettingsForm
      v-else
      :settings="state.settings?.backend ?? null"
      @update="updateBackendSettings"
      @update-fun-speech="updateFunSpeechSettings"
      @update-mcp="updateMcpSettings"
      @commit="saveSettings"
    />
  </section>
</template>
