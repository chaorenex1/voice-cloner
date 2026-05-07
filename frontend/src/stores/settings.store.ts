import { reactive } from 'vue';
import {
  checkBackendHealth,
  getSettings,
  listAudioDevices,
  updateSettings,
} from '../services/tauri/settings';
import type {
  AppSettings,
  AudioDeviceSnapshot,
  BackendEndpointConfig,
  BackendHealthSnapshot,
  DeviceSettings,
  SettingsSection,
} from '../utils/types/settings';

export interface SettingsState {
  settings: AppSettings | null;
  audioDevices: AudioDeviceSnapshot | null;
  health: BackendHealthSnapshot[];
  activeSection: SettingsSection;
  loading: boolean;
  saving: boolean;
  lastMessage: string;
}

const state = reactive<SettingsState>({
  settings: null,
  audioDevices: null,
  health: [],
  activeSection: 'devices',
  loading: false,
  saving: false,
  lastMessage: '设置等待加载',
});

export function useSettingsStore() {
  async function loadSettings(): Promise<void> {
    state.loading = true;
    try {
      const [settings, audioDevices] = await Promise.all([getSettings(), listAudioDevices()]);
      state.settings = settings;
      state.audioDevices = audioDevices;
      state.health = [
        {
          service: 'funspeech',
          status: 'idle',
          message: '等待测试 FunSpeech 连接',
        },
        {
          service: 'llm',
          status: 'idle',
          message: '等待测试 LLM 后端连接',
        },
      ];
      state.lastMessage = '设置已加载';
    } finally {
      state.loading = false;
    }
  }

  function setSection(section: SettingsSection): void {
    state.activeSection = section;
  }

  function updateDeviceSettings(patch: Partial<DeviceSettings>): void {
    if (!state.settings) {
      return;
    }

    state.settings = {
      ...state.settings,
      devices: { ...state.settings.devices, ...patch },
    };
  }

  function updateBackendSettings(
    key: keyof AppSettings['backends'],
    patch: Partial<BackendEndpointConfig>
  ): void {
    if (!state.settings) {
      return;
    }

    state.settings = {
      ...state.settings,
      backends: {
        ...state.settings.backends,
        [key]: { ...state.settings.backends[key], ...patch },
      },
    };
  }

  async function saveSettings(): Promise<AppSettings | null> {
    if (!state.settings) {
      return null;
    }

    state.saving = true;
    try {
      state.settings = await updateSettings(state.settings);
      state.lastMessage = '设置已保存';
      return state.settings;
    } finally {
      state.saving = false;
    }
  }

  async function testConnections(): Promise<void> {
    state.health = state.health.map((item) => ({
      ...item,
      status: 'checking',
      message: '正在测试连接...',
    }));

    const result = await checkBackendHealth({ services: ['funspeech', 'llm'] });
    state.health = result.health;
    state.lastMessage = '连接测试完成';
  }

  return {
    state,
    loadSettings,
    setSection,
    updateDeviceSettings,
    updateBackendSettings,
    saveSettings,
    testConnections,
  };
}
