<script setup lang="ts">
import { onMounted } from 'vue';
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
  saveSettings,
  testConnections,
} = useSettingsStore();

// const activeTitle = computed(() => (state.activeSection === 'devices' ? '设备设置' : '后端设置'));

onMounted(() => {
  if (!state.settings) {
    void loadSettings();
  }
});
</script>

<template>
  <section class="module-page settings-page">
    <header class="page-toolbar settings-toolbar">
      <div>
        <p class="module-eyebrow">Settings</p>
        <!-- <h2>{{ activeTitle }}</h2> -->
        <p class="module-description">管理音频设备、虚拟麦克风、FunSpeech 和 LLM 后端连接。</p>
      </div>
    </header>

    <SettingsSectionTabs :active-section="state.activeSection" @change="setSection" />

    <div v-if="state.activeSection === 'devices'" class="settings-layout">
      <DeviceSettingsForm
        :devices="state.audioDevices"
        :settings="state.settings?.devices ?? null"
        @update="updateDeviceSettings"
      />
    </div>

    <BackendSettingsForm
      v-else
      :settings="state.settings?.backends ?? null"
      @update="updateBackendSettings"
    />

    <footer class="settings-action-bar">
      <span>{{ state.lastMessage }}</span>
      <div>
        <button
          class="ghost-button"
          type="button"
          :disabled="state.loading"
          @click="testConnections"
        >
          测试连接
        </button>
        <button class="primary-button" type="button" :disabled="state.saving" @click="saveSettings">
          保存设置
        </button>
      </div>
    </footer>
  </section>
</template>
