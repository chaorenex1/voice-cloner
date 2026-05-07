export type SettingsSection = 'devices' | 'backends';

export type AudioDeviceKind = 'input' | 'output' | 'virtualMic';

export type BackendHealthStatus = 'idle' | 'checking' | 'ok' | 'warning' | 'error';

export interface AudioDevice {
  id: string;
  label: string;
  kind: AudioDeviceKind;
  isDefault?: boolean;
}

export interface AudioDeviceSnapshot {
  inputDevices: AudioDevice[];
  outputDevices: AudioDevice[];
  virtualMicDevices: AudioDevice[];
}

export interface DeviceSettings {
  inputDeviceId: string | null;
  outputDeviceId: string | null;
  virtualMicDeviceId: string | null;
  virtualMicEnabled: boolean;
}

export interface BackendEndpointConfig {
  providerName: string;
  baseUrl: string;
  apiKeyRef: string | null;
  model: string | null;
  timeoutMs: number;
  region: string | null;
  extraOptions: Record<string, unknown>;
}

export interface BackendSettings {
  funspeech: BackendEndpointConfig;
  llm: BackendEndpointConfig;
}

export interface AppSettings {
  devices: DeviceSettings;
  backends: BackendSettings;
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
