import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type {
  SyncVoicesRequest,
  VoiceDetail,
  VoiceMutationResult,
  VoiceSummary,
  VoiceSyncResult,
  VoiceSyncStatus,
} from '../../utils/types/voice';

interface CustomVoiceProfileView {
  voiceName: string;
  sourcePromptText?: string | null;
  voiceInstruction: string;
  referenceText: string;
  hasReferenceAudio: boolean;
  referenceAudioFileName?: string | null;
  syncStatus: 'localOnly' | 'pendingSync' | 'synced' | 'failed' | 'conflict';
  lastSyncedAt: string | null;
  createdAt: string;
}

interface VoiceSyncReport {
  operation: string;
  voiceName?: string | null;
  localVoiceCount: number;
  syncStatus?: 'localOnly' | 'pendingSync' | 'synced' | 'failed' | 'conflict' | null;
  message: string;
}

export interface WavUploadPayload {
  fileName: string;
  bytes: number[];
}

export interface ReferenceAudioTranscription {
  fileName: string;
  text: string;
}

export interface VoicePreviewFinishedEvent {
  voiceName: string;
  playingVoiceName: string | null;
}

export interface CreateCustomVoiceRequest {
  displayName: string;
  referenceText: string;
  voiceInstruction?: string;
  upload: WavUploadPayload;
}

let voiceListCache: VoiceSummary[] | null = null;
let voiceListPromise: Promise<VoiceSummary[]> | null = null;

function cloneVoiceSummaries(voices: VoiceSummary[]): VoiceSummary[] {
  return structuredClone(voices);
}

function invalidateCustomVoiceCache(): void {
  voiceListCache = null;
  voiceListPromise = null;
}

function voiceNameFromDisplayName(displayName: string): string {
  const voiceName = displayName.trim();
  if (!voiceName) {
    throw new Error('voiceName is required');
  }
  return voiceName;
}

function syncStatusFromProfile(status: CustomVoiceProfileView['syncStatus']): VoiceSyncStatus {
  switch (status) {
    case 'synced':
      return 'synced';
    case 'failed':
      return 'failed';
    case 'conflict':
      return 'remoteChanged';
    case 'pendingSync':
    case 'localOnly':
    default:
      return 'localOnly';
  }
}

function syncStatusFromReport(status: VoiceSyncReport['syncStatus']): VoiceSyncStatus {
  switch (status) {
    case 'synced':
      return 'synced';
    case 'failed':
      return 'failed';
    case 'conflict':
      return 'remoteChanged';
    default:
      return 'localOnly';
  }
}

function summaryFromProfile(profile: CustomVoiceProfileView): VoiceSummary {
  const isRemoteImport = profile.sourcePromptText === 'funspeechRemote';
  return {
    voiceName: profile.voiceName,
    displayName: profile.voiceName,
    source: isRemoteImport ? 'remote' : 'custom',
    tags: [isRemoteImport ? '云端' : '自定义', profile.syncStatus === 'synced' ? '已同步' : '本地'],
    hasReferenceAudio: profile.hasReferenceAudio,
    updatedAt: profile.lastSyncedAt ?? profile.createdAt,
    referenceTextPreview: profile.referenceText.slice(0, 42),
    syncStatus: syncStatusFromProfile(profile.syncStatus),
  };
}

function detailFromProfile(profile: CustomVoiceProfileView): VoiceDetail {
  return {
    ...summaryFromProfile(profile),
    voiceInstruction: profile.voiceInstruction,
    referenceText: profile.referenceText,
    referenceAudioFileName: profile.referenceAudioFileName ?? undefined,
    editable: profile.sourcePromptText !== 'funspeechRemote',
  };
}

async function listCachedVoiceSummaries(): Promise<VoiceSummary[]> {
  if (voiceListCache) {
    return cloneVoiceSummaries(voiceListCache);
  }

  voiceListPromise ??= invoke<CustomVoiceProfileView[]>('list_custom_voices')
    .then((profiles) => {
      const summaries = profiles.map((profile) => summaryFromProfile(profile));
      voiceListCache = cloneVoiceSummaries(summaries);
      return cloneVoiceSummaries(summaries);
    })
    .finally(() => {
      voiceListPromise = null;
    });

  return cloneVoiceSummaries(await voiceListPromise);
}

export async function listVoices(): Promise<VoiceSummary[]> {
  return listCachedVoiceSummaries();
}

export async function getVoiceDetail(voiceName: string): Promise<VoiceDetail> {
  try {
    const profile = await invoke<CustomVoiceProfileView>('get_custom_voice', { voiceName });
    return detailFromProfile(profile);
  } catch (_error) {
    throw new Error(`音色不存在：${voiceName}`);
  }
}

async function saveProfile(
  detail: VoiceDetail,
  upload: WavUploadPayload | null
): Promise<CustomVoiceProfileView> {
  return invoke<CustomVoiceProfileView>('save_custom_voice_profile', {
    request: {
      voiceName: detail.voiceName,
      voiceInstruction: detail.voiceInstruction ?? '',
      referenceText: detail.referenceText,
      referenceAudioFileName: upload?.fileName ?? null,
      referenceAudioBytes: upload?.bytes ?? null,
    },
  });
}

async function syncVoice(
  voiceName: string,
  operation: 'register' | 'update'
): Promise<VoiceSyncReport> {
  const command = operation === 'register' ? 'register_custom_voice' : 'update_custom_voice_sync';
  return invoke<VoiceSyncReport>(command, { voiceName });
}

export async function createCustomVoice(
  request: CreateCustomVoiceRequest
): Promise<VoiceMutationResult> {
  const voiceName = voiceNameFromDisplayName(request.displayName);
  const detail: VoiceDetail = {
    voiceName,
    displayName: voiceName,
    source: 'custom',
    tags: ['自定义', '本地'],
    hasReferenceAudio: true,
    updatedAt: new Date().toLocaleString('zh-CN', { hour12: false }),
    referenceTextPreview: request.referenceText.slice(0, 42),
    syncStatus: 'localOnly',
    voiceInstruction: request.voiceInstruction ?? '',
    referenceText: request.referenceText,
    referenceAudioFileName: request.upload.fileName,
    editable: true,
  };
  const saved = await saveProfile(detail, request.upload);
  const report = await syncVoice(saved.voiceName, 'register');
  invalidateCustomVoiceCache();
  return {
    voiceName: saved.voiceName,
    message: report.message,
    updatedAt: saved.lastSyncedAt ?? saved.createdAt,
    syncStatus: syncStatusFromReport(report.syncStatus),
  };
}

export async function saveVoiceDetail(
  detail: VoiceDetail,
  upload: WavUploadPayload | null
): Promise<VoiceMutationResult> {
  const saved = await saveProfile(detail, upload);
  const report = await syncVoice(saved.voiceName, 'update');
  invalidateCustomVoiceCache();
  return {
    voiceName: saved.voiceName,
    message: report.message,
    updatedAt: saved.lastSyncedAt ?? saved.createdAt,
    syncStatus: syncStatusFromReport(report.syncStatus),
  };
}

export async function deleteVoice(voiceName: string): Promise<VoiceMutationResult> {
  const report = await invoke<VoiceSyncReport>('delete_custom_voice_sync', { voiceName });
  invalidateCustomVoiceCache();
  return {
    voiceName,
    message: report.message,
    updatedAt: new Date().toLocaleString('zh-CN', { hour12: false }),
    syncStatus: syncStatusFromReport(report.syncStatus),
  };
}

export async function syncVoices(request: SyncVoicesRequest): Promise<VoiceSyncResult> {
  const report =
    request.mode === 'full'
      ? await invoke<VoiceSyncReport>('sync_voices_full')
      : await invoke<VoiceSyncReport>('refresh_voice_runtime');
  invalidateCustomVoiceCache();
  return {
    mode: request.mode,
    syncedCount: report.syncStatus === 'failed' ? 0 : report.localVoiceCount,
    failedCount: report.syncStatus === 'failed' ? 1 : 0,
    message: report.message,
  };
}

export interface VoicePreviewState {
  playingVoiceName: string | null;
}

export async function toggleVoicePreview(detail: VoiceDetail): Promise<VoicePreviewState> {
  if (!detail.hasReferenceAudio || detail.source === 'remote') {
    throw new Error('该音色没有可试听的本地 wav 文件');
  }
  return invoke<VoicePreviewState>('toggle_voice_preview', {
    request: {
      voiceName: detail.voiceName,
    },
  });
}

export async function stopVoicePreview(): Promise<VoicePreviewState> {
  return invoke<VoicePreviewState>('stop_voice_preview');
}

export async function transcribeReferenceAudio(
  upload: WavUploadPayload
): Promise<ReferenceAudioTranscription> {
  return invoke<ReferenceAudioTranscription>('transcribe_reference_audio', {
    request: {
      fileName: upload.fileName,
      audioBytes: upload.bytes,
    },
  });
}

export function listenVoicePreviewFinished(
  handler: (event: VoicePreviewFinishedEvent) => void
): Promise<() => void> {
  return listen<VoicePreviewFinishedEvent>('voice-preview-finished', (event) => {
    handler(event.payload);
  });
}
