import type { UnlistenFn } from '@tauri-apps/api/event';

export type VoiceSeparationSourceType = 'video' | 'audio';
export type VoiceSeparationStatus =
  | 'queued'
  | 'extractingAudio'
  | 'decoding'
  | 'separating'
  | 'mixingNoVocals'
  | 'postProcessing'
  | 'ready'
  | 'savingVoice'
  | 'saved'
  | 'failed'
  | 'cancelled';
export type VoiceSeparationModel = 'htDemucs' | 'htDemucsFt';
export type VoiceSeparationStemName = 'vocals' | 'noVocals' | 'drums' | 'bass' | 'other';
export type DenoiseMode = 'off' | 'standard' | 'strong';
export type AudioChannelMode = 'mono' | 'stereo';

export interface VoicePostProcessConfig {
  trimSilence: boolean;
  denoiseMode: DenoiseMode;
  targetSampleRate: number;
  channels: AudioChannelMode;
  loudnessNormalization: boolean;
  targetLufs: number;
  truePeakDb: number;
  peakLimiter: boolean;
}

export interface AudioPostProcessReport {
  inputDurationSeconds: number;
  outputDurationSeconds: number;
  inputSampleRate: number;
  outputSampleRate: number;
  inputChannels: number;
  outputChannels: number;
  denoiseApplied: boolean;
  trimApplied: boolean;
  loudnessApplied: boolean;
  peakDb: number;
  rmsDb: number;
  warnings: string[];
}

export interface VoiceSeparationStems {
  vocals?: string | null;
  noVocals?: string | null;
  drums?: string | null;
  bass?: string | null;
  other?: string | null;
}

export interface VoiceSeparationJob {
  jobId: string;
  traceId: string;
  sourceType: VoiceSeparationSourceType;
  sourcePath: string;
  sourceFileName: string;
  model: VoiceSeparationModel;
  status: VoiceSeparationStatus;
  progress: number;
  currentStageMessage: string;
  decodedAudioPath?: string | null;
  stems?: VoiceSeparationStems | null;
  postProcessedVocalsPath?: string | null;
  postProcessReport?: AudioPostProcessReport | null;
  referenceText?: string | null;
  voiceName?: string | null;
  errorMessage?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface VoiceSeparationRuntimeStatus {
  ffmpegAvailable: boolean;
  ffmpegVersion?: string | null;
  demucsRsAvailable: boolean;
  demucsRsVersion?: string | null;
  defaultModelAvailable: boolean;
  modelCachePath?: string | null;
  gpuBackend?: string | null;
  warnings: string[];
}

export interface CreateVoiceSeparationJobRequest {
  sourcePath: string;
  model?: VoiceSeparationModel;
  postProcessConfig?: VoicePostProcessConfig;
}

export interface StartVoiceSeparationJobRequest {
  postProcessConfig?: VoicePostProcessConfig;
}

export interface VoiceSeparationPreviewState {
  playingJobId?: string | null;
  playingStem?: VoiceSeparationStemName | null;
}

export interface ReferenceAudioTranscription {
  fileName: string;
  text: string;
}

export interface VoiceSeparationDownloadResult {
  targetPath: string;
}

export interface SaveSeparatedVocalsRequest {
  voiceName: string;
  referenceText: string;
  voiceInstruction?: string;
}

export interface CustomVoiceProfileResult {
  voiceName: string;
  sourcePromptText?: string | null;
  asrText?: string | null;
  voiceInstruction: string;
  referenceAudioPath: string;
  referenceText: string;
  syncStatus: 'localOnly' | 'pendingSync' | 'synced' | 'failed' | 'conflict';
  lastSyncedAt?: string | null;
  createdAt: string;
}

export type VoiceSeparationUnlisten = UnlistenFn;

export const lufsPresetOptions = [
  { label: '保守 / ASR（-18 LUFS）', value: -18 },
  { label: '短视频 / 播客（-16 LUFS）', value: -16 },
  { label: '音乐人声（-14 LUFS）', value: -14 },
  { label: '更响音乐人声（-12 LUFS）', value: -12 },
] as const;

export const defaultPostProcessConfig: VoicePostProcessConfig = {
  trimSilence: false,
  denoiseMode: 'standard',
  targetSampleRate: 48000,
  channels: 'mono',
  loudnessNormalization: true,
  targetLufs: -18,
  truePeakDb: -1.5,
  peakLimiter: true,
};

export const defaultStereoPostProcessConfig: VoicePostProcessConfig = {
  ...defaultPostProcessConfig,
  channels: 'stereo',
};
