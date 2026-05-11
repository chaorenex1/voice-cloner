import type { VoicePostProcessConfig } from './voice-separation';

export type OfflineInputType = 'audio' | 'text';

export type OfflineJobStatus = 'created' | 'running' | 'completed' | 'failed' | 'cancelled';

export interface RuntimeParams {
  values: Record<string, unknown>;
}

export interface TtsEmotionOption {
  id: string;
  label: string;
  prompt: string;
}

export interface TtsEmotionOptions {
  supportsEmotionControl: boolean;
  emotions: TtsEmotionOption[];
}

export interface OfflineJob {
  jobId: string;
  traceId: string;
  inputType: OfflineInputType;
  inputRef: string;
  inputFileName: string | null;
  voiceName: string;
  runtimeParams: RuntimeParams;
  postProcessConfig?: VoicePostProcessConfig | null;
  outputFormat: 'wav';
  status: OfflineJobStatus;
  stage: string;
  progress: number;
  artifactUrl: string | null;
  localArtifactPath: string | null;
  errorSummary: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface CreateOfflineAudioJobRequest {
  fileName?: string;
  inputRef?: string;
  inputBytes?: number[];
  voiceName: string;
  runtimeParams: RuntimeParams;
  postProcessConfig?: VoicePostProcessConfig;
  outputFormat?: 'wav';
}

export interface CreateOfflineTextJobRequest {
  text: string;
  voiceName: string;
  runtimeParams: RuntimeParams;
  postProcessConfig?: VoicePostProcessConfig;
  outputFormat?: 'wav';
}
