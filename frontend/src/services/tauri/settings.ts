import type {
  AppSettings,
  AudioDeviceSnapshot,
  BackendHealthRequest,
  BackendHealthResult,
} from '../../utils/types/settings';
import { invokeWithMockFallback } from './invoke';

const mockSettings: AppSettings = {
  devices: {
    inputDeviceId: 'shure_mv7',
    outputDeviceId: 'headphones',
    virtualMicDeviceId: 'vb_cable',
    virtualMicEnabled: true,
  },
  backends: {
    funspeech: {
      providerName: 'FunSpeech',
      baseUrl: 'http://127.0.0.1:8000',
      apiKeyRef: 'local/funspeech/default',
      model: null,
      timeoutMs: 10000,
      region: null,
      extraOptions: {},
    },
    llm: {
      providerName: 'LLM',
      baseUrl: 'http://127.0.0.1:11434',
      apiKeyRef: null,
      model: 'qwen2.5:latest',
      timeoutMs: 12000,
      region: null,
      extraOptions: {},
    },
  },
};

const mockAudioDevices: AudioDeviceSnapshot = {
  inputDevices: [
    { id: 'shure_mv7', label: 'Shure MV7', kind: 'input', isDefault: true },
    { id: 'macbook_mic', label: 'Built-in Microphone', kind: 'input' },
    { id: 'usb_interface', label: 'USB Audio Interface', kind: 'input' },
  ],
  outputDevices: [
    { id: 'headphones', label: 'Headphones', kind: 'output', isDefault: true },
    { id: 'studio_monitor', label: 'Studio Monitor', kind: 'output' },
    { id: 'built_in_speaker', label: 'Built-in Speaker', kind: 'output' },
  ],
  virtualMicDevices: [
    { id: 'vb_cable', label: 'VB-Cable Input', kind: 'virtualMic', isDefault: true },
    { id: 'blackhole', label: 'BlackHole 2ch', kind: 'virtualMic' },
  ],
};

export async function getSettings(): Promise<AppSettings> {
  return invokeWithMockFallback('get_settings', () => structuredClone(mockSettings));
}

export async function updateSettings(settings: AppSettings): Promise<AppSettings> {
  return invokeWithMockFallback('update_settings', () => structuredClone(settings), {
    input: { section: 'all', payload: settings },
  });
}

export async function listAudioDevices(): Promise<AudioDeviceSnapshot> {
  return invokeWithMockFallback('list_audio_devices', () => structuredClone(mockAudioDevices));
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
        message: index === 0 ? 'reachable' : 'mock response; backend command not wired yet',
        checkedAt: new Date().toLocaleString('zh-CN', { hour12: false }),
      })),
    }),
    { request }
  );
}
