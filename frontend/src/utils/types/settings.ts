export type SettingsSection = 'devices' | 'backends';

export type AudioDeviceKind = 'input' | 'output';

export type RealtimeVoiceMode = 'realtimeVoice';

export type BackendHealthStatus = 'idle' | 'checking' | 'ok' | 'warning' | 'error';

export interface AudioDevice {
  id: string;
  name: string;
  kind: AudioDeviceKind;
  isDefault: boolean;
}

export interface AudioDeviceSnapshot {
  inputDevices: AudioDevice[];
  outputDevices: AudioDevice[];
}

export interface DeviceSettings {
  inputDeviceId: string | null;
  outputDeviceId: string | null;
  monitorEnabled: boolean;
  virtualMicEnabled: boolean;
  virtualMicDeviceId: string | null;
}

export interface BackendEndpointConfig {
  baseUrl: string;
  apiKeyRef: string | null;
  model: string | null;
  timeoutMs: number;
  region: string | null;
  extraOptions: Record<string, string>;
}

export interface McpSettings {
  enabled: boolean;
  host: string;
  port: number;
  path: string;
}

export interface BackendSettings {
  llm: BackendEndpointConfig;
  asr: BackendEndpointConfig;
  tts: BackendEndpointConfig;
  realtime: BackendEndpointConfig;
  mcp: McpSettings;
}

export interface RuntimeSettings {
  defaultOutputFormat: string;
  defaultSampleRate: number;
  audioFrameMs: number;
  realtimeVoiceMode: RealtimeVoiceMode;
  realtimeDebugEnabled: boolean;
  realtimePlaybackAckEnabled: boolean;
}

export interface AppSettings {
  device: DeviceSettings;
  backend: BackendSettings;
  runtime: RuntimeSettings;
}

export interface BackendHealthSnapshot {
  service: string;
  status: BackendHealthStatus;
  latencyMs?: number;
  message: string;
  checkedAt?: string;
}

export interface BackendHealthRequest {
  services: string[];
}

export interface BackendHealthResult {
  health: BackendHealthSnapshot[];
}
