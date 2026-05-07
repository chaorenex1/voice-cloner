import type {
  AppSettings,
  AudioDevice,
  AudioDeviceSnapshot,
  BackendHealthRequest,
  BackendHealthResult,
} from '../../utils/types/settings';
import { invokeWithMockFallback } from './invoke';

const mockSettings: AppSettings = {
  device: {
    inputDeviceId: 'shure_mv7',
    outputDeviceId: 'headphones',
    monitorEnabled: true,
    virtualMicEnabled: true,
    virtualMicDeviceId: 'vb_cable_input',
  },
  backend: {
    llm: {
      baseUrl: 'http://127.0.0.1:11434',
      apiKeyRef: null,
      model: 'qwen2.5:latest',
      timeoutMs: 12000,
      region: null,
      extraOptions: {},
    },
    asr: {
      baseUrl: 'http://127.0.0.1:8000',
      apiKeyRef: null,
      model: null,
      timeoutMs: 10000,
      region: null,
      extraOptions: {},
    },
    tts: {
      baseUrl: 'http://127.0.0.1:8000',
      apiKeyRef: null,
      model: null,
      timeoutMs: 10000,
      region: null,
      extraOptions: {},
    },
    realtime: {
      baseUrl: 'http://127.0.0.1:8000',
      apiKeyRef: null,
      model: null,
      timeoutMs: 10000,
      region: null,
      extraOptions: {},
    },
  },
  runtime: {
    defaultVoiceName: null,
    defaultOutputFormat: 'wav',
    defaultSampleRate: 48000,
    audioFrameMs: 20,
  },
};

const mockAudioDevices: AudioDeviceSnapshot = {
  inputDevices: [
    { id: 'shure_mv7', name: 'Shure MV7', kind: 'input', isDefault: true },
    { id: 'vb_cable_input', name: 'VB-Cable Virtual Microphone', kind: 'input', isDefault: false },
    { id: 'macbook_mic', name: 'Built-in Microphone', kind: 'input', isDefault: false },
    { id: 'usb_interface', name: 'USB Audio Interface', kind: 'input', isDefault: false },
  ],
  outputDevices: [
    { id: 'headphones', name: 'Headphones', kind: 'output', isDefault: true },
    { id: 'studio_monitor', name: 'Studio Monitor', kind: 'output', isDefault: false },
    { id: 'built_in_speaker', name: 'Built-in Speaker', kind: 'output', isDefault: false },
  ],
};

let cachedSettings: AppSettings | null = null;
let settingsLoadPromise: Promise<AppSettings> | null = null;

function cloneSettings(settings: AppSettings): AppSettings {
  return structuredClone(settings);
}

export async function getSettings(): Promise<AppSettings> {
  if (cachedSettings) {
    return cloneSettings(cachedSettings);
  }

  settingsLoadPromise ??= invokeWithMockFallback('get_app_settings', () =>
    cloneSettings(mockSettings)
  )
    .then((settings) => {
      cachedSettings = cloneSettings(settings);
      return cloneSettings(settings);
    })
    .finally(() => {
      settingsLoadPromise = null;
    });

  return cloneSettings(await settingsLoadPromise);
}

export async function updateSettings(settings: AppSettings): Promise<AppSettings> {
  const nextSettings: AppSettings = {
    ...settings,
    backend: {
      ...settings.backend,
      asr: { ...settings.backend.asr, model: null },
      tts: { ...settings.backend.tts, model: null },
      realtime: { ...settings.backend.realtime, model: null },
    },
  };

  const saved = await invokeWithMockFallback(
    'update_app_settings',
    () => cloneSettings(nextSettings),
    {
      patch: {
        device: nextSettings.device,
        backend: nextSettings.backend,
        runtime: nextSettings.runtime,
      },
    }
  );
  cachedSettings = cloneSettings(saved);
  return cloneSettings(saved);
}

export async function listAudioDevices(): Promise<AudioDeviceSnapshot> {
  try {
    const [inputDevices, outputDevices] = await Promise.all([
      invokeWithMockFallback<AudioDevice[]>('list_audio_input_devices', () =>
        structuredClone(mockAudioDevices.inputDevices)
      ),
      invokeWithMockFallback<AudioDevice[]>('list_audio_output_devices', () =>
        structuredClone(mockAudioDevices.outputDevices)
      ),
    ]);

    return { inputDevices, outputDevices };
  } catch (_error) {
    return structuredClone(mockAudioDevices);
  }
}

export async function checkBackendHealth(
  request: BackendHealthRequest
): Promise<BackendHealthResult> {
  return invokeWithMockFallback(
    'check_backend_health',
    () => ({
      health: request.services.map((service, index) => ({
        service,
        status: index === 0 ? 'ok' : 'warning',
        latencyMs: index === 0 ? 86 : 141,
        message: index === 0 ? 'reachable' : 'frontend preview mock response',
        checkedAt: new Date().toLocaleString('zh-CN', { hour12: false }),
      })),
    }),
    { request }
  );
}
