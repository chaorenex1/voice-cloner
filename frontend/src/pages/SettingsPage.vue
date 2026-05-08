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
        <p class="module-description">管理音频设备、虚拟麦克风、FunSpeech 和 LLM 后端连接。</p>
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
    </div>

    <BackendSettingsForm
      v-else
      :settings="state.settings?.backend ?? null"
      :runtime="state.settings?.runtime ?? null"
      @update="updateBackendSettings"
      @update-fun-speech="updateFunSpeechSettings"
      @update-runtime="updateRuntimeSettings"
      @commit="saveSettings"
    />
  </section>
</template>
