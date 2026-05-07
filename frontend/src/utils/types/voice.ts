export type VoiceSource = 'preset' | 'custom' | 'remote';

export type VoiceSyncStatus = 'synced' | 'localOnly' | 'remoteChanged' | 'failed';

export type VoiceSyncMode = 'full' | 'incremental' | 'retryFailed';

export interface VoiceSummary {
  voiceName: string;
  displayName: string;
  source: VoiceSource;
  tags: string[];
  hasReferenceAudio: boolean;
  updatedAt: string;
  referenceTextPreview: string;
  syncStatus: VoiceSyncStatus;
  isCurrent: boolean;
}

export interface VoiceDetail extends VoiceSummary {
  voiceInstruction?: string;
  referenceText: string;
  referenceAudioPath?: string;
  referenceAudioFileName?: string;
  previewAudioPath?: string;
  editable: boolean;
}

export interface VoiceMutationResult {
  voiceName: string;
  message: string;
  updatedAt: string;
  syncStatus: VoiceSyncStatus;
}

export interface SyncVoicesRequest {
  mode: VoiceSyncMode;
  voiceNames?: string[];
}

export interface VoiceSyncResult {
  mode: VoiceSyncMode;
  syncedCount: number;
  failedCount: number;
  message: string;
}
