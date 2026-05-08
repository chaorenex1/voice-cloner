import { reactive } from 'vue';
import { getSettings, listAudioDevices, updateSettings } from '../services/tauri/settings';
import type {
  AppSettings,
  AudioDeviceSnapshot,
  BackendEndpointConfig,
  BackendSettings,
  DeviceSettings,
  RuntimeSettings,
  SettingsSection,
} from '../utils/types/settings';

const funSpeechKeys: Array<Exclude<keyof BackendSettings, 'llm'>> = ['asr', 'tts', 'realtime'];

export interface SettingsState {
  settings: AppSettings | null;
  audioDevices: AudioDeviceSnapshot | null;
  activeSection: SettingsSection;
  loading: boolean;
  saving: boolean;
  lastMessage: string;
}

const state = reactive<SettingsState>({
  settings: null,
  audioDevices: null,
  activeSection: 'devices',
  loading: false,
  saving: false,
  lastMessage: '设置等待加载',
});

let dirtyRevision = 0;

function cloneSettings(settings: AppSettings): AppSettings {
  return JSON.parse(JSON.stringify(settings)) as AppSettings;
}

export function useSettingsStore() {
  async function loadSettings(): Promise<void> {
    state.loading = true;
    try {
      const [settings, audioDevices] = await Promise.all([getSettings(), listAudioDevices()]);
      state.settings = settings;
      state.audioDevices = audioDevices;
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
      device: { ...state.settings.device, ...patch },
    };
    dirtyRevision += 1;
  }

  function updateBackendSettings(
    key: keyof AppSettings['backend'],
    patch: Partial<BackendEndpointConfig>
  ): void {
    if (!state.settings) {
      return;
    }

    state.settings = {
      ...state.settings,
      backend: {
        ...state.settings.backend,
        [key]: { ...state.settings.backend[key], ...patch },
      },
    };
    dirtyRevision += 1;
  }

  function updateFunSpeechSettings(patch: Partial<BackendEndpointConfig>): void {
    if (!state.settings) {
      return;
    }

    const sanitizedPatch = { ...patch, model: null };
    state.settings = {
      ...state.settings,
      backend: funSpeechKeys.reduce(
        (backend, key) => ({
          ...backend,
          [key]: { ...backend[key], ...sanitizedPatch },
        }),
        { ...state.settings.backend }
      ),
    };
    dirtyRevision += 1;
  }

  function updateRuntimeSettings(patch: Partial<RuntimeSettings>): void {
    if (!state.settings) {
      return;
    }

    state.settings = {
      ...state.settings,
      runtime: { ...state.settings.runtime, ...patch },
    };
    dirtyRevision += 1;
  }

  async function saveSettings(): Promise<AppSettings | null> {
    if (!state.settings) {
      return null;
    }

    const revisionAtSave = dirtyRevision;
    const snapshot = cloneSettings(state.settings);
    state.saving = true;
    try {
      const saved = await updateSettings(snapshot);
      if (dirtyRevision === revisionAtSave) {
        state.settings = saved;
        state.lastMessage = '设置已保存到本地';
      }
      return saved;
    } catch (error) {
      state.lastMessage = `设置保存失败：${error instanceof Error ? error.message : String(error)}`;
      return null;
    } finally {
      state.saving = false;
    }
  }

  return {
    state,
    loadSettings,
    setSection,
    updateDeviceSettings,
    updateBackendSettings,
    updateFunSpeechSettings,
    updateRuntimeSettings,
    saveSettings,
  };
}
