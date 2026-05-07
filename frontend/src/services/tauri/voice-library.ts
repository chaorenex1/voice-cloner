import { invoke } from '@tauri-apps/api/core';
import type {
  SyncVoicesRequest,
  VoiceDetail,
  VoiceMutationResult,
  VoiceSource,
  VoiceSummary,
  VoiceSyncResult,
  VoiceSyncStatus,
} from '../../utils/types/voice';
import { getSettings, updateSettings } from './settings';

interface CustomVoiceProfile {
  voiceName: string;
  voiceInstruction: string;
  referenceAudioPath: string;
  referenceText: string;
  syncStatus: 'localOnly' | 'pendingSync' | 'synced' | 'failed' | 'conflict';
  lastSyncedAt: string | null;
  createdAt: string;
}

interface RemoteVoiceInfo {
  voiceName: string;
  type?: string;
  referenceText?: string;
  referenceAudio?: string;
  voiceInstruction?: string;
  status?: string;
  updatedAt?: string;
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

export interface CreateCustomVoiceRequest {
  displayName: string;
  referenceText: string;
  voiceInstruction?: string;
  upload: WavUploadPayload;
}

const remoteVoiceCache = new Map<string, VoiceDetail>();

function voiceNameFromDisplayName(displayName: string): string {
  return (
    displayName
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9_-]+/gi, '-')
      .replace(/^-+|-+$/g, '') || `voice-${Date.now()}`
  );
}

function syncStatusFromProfile(status: CustomVoiceProfile['syncStatus']): VoiceSyncStatus {
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

function detailToSummary(detail: VoiceDetail): VoiceSummary {
  return {
    voiceName: detail.voiceName,
    displayName: detail.displayName,
    source: detail.source,
    tags: detail.tags,
    hasReferenceAudio: detail.hasReferenceAudio,
    updatedAt: detail.updatedAt,
    referenceTextPreview: detail.referenceTextPreview,
    syncStatus: detail.syncStatus,
    isCurrent: detail.isCurrent,
  };
}

function detailFromProfile(
  profile: CustomVoiceProfile,
  currentVoiceName: string | null
): VoiceDetail {
  const referenceTextPreview = profile.referenceText.slice(0, 42);
  return {
    voiceName: profile.voiceName,
    displayName: profile.voiceName,
    source: 'custom',
    tags: ['自定义', profile.syncStatus === 'synced' ? '已同步' : '本地'],
    hasReferenceAudio: Boolean(profile.referenceAudioPath),
    updatedAt: profile.lastSyncedAt ?? profile.createdAt,
    referenceTextPreview,
    syncStatus: syncStatusFromProfile(profile.syncStatus),
    isCurrent: currentVoiceName === profile.voiceName,
    voiceInstruction: profile.voiceInstruction,
    referenceText: profile.referenceText,
    referenceAudioPath: profile.referenceAudioPath,
    referenceAudioFileName: `${profile.voiceName}.wav`,
    editable: true,
  };
}

function detailFromRemote(remote: RemoteVoiceInfo, currentVoiceName: string | null): VoiceDetail {
  const source: VoiceSource = remote.type === 'preset' ? 'preset' : 'remote';
  const referenceText = remote.referenceText ?? '';
  return {
    voiceName: remote.voiceName,
    displayName: remote.voiceName,
    source,
    tags: [source === 'preset' ? '预置' : '云端', remote.status ?? 'active'],
    hasReferenceAudio: Boolean(remote.referenceAudio),
    updatedAt: remote.updatedAt || 'FunSpeech',
    referenceTextPreview: referenceText.slice(0, 42),
    syncStatus: 'remoteChanged',
    isCurrent: currentVoiceName === remote.voiceName,
    voiceInstruction: remote.voiceInstruction ?? '',
    referenceText,
    referenceAudioPath: remote.referenceAudio,
    referenceAudioFileName: remote.referenceAudio
      ? remote.referenceAudio.split(/[\\/]/).pop()
      : undefined,
    editable: false,
  };
}

async function currentVoiceName(): Promise<string | null> {
  try {
    return (await getSettings()).runtime.defaultVoiceName;
  } catch (_error) {
    return null;
  }
}

async function remoteVoices(): Promise<RemoteVoiceInfo[]> {
  try {
    return await invoke<RemoteVoiceInfo[]>('list_remote_voices');
  } catch (_error) {
    return [];
  }
}

export async function listVoices(): Promise<VoiceSummary[]> {
  const [localProfiles, remoteProfiles, current] = await Promise.all([
    invoke<CustomVoiceProfile[]>('list_custom_voices'),
    remoteVoices(),
    currentVoiceName(),
  ]);

  remoteVoiceCache.clear();
  const localDetails = localProfiles.map((profile) => detailFromProfile(profile, current));
  const byName = new Map<string, VoiceDetail>();
  for (const detail of localDetails) {
    byName.set(detail.voiceName, detail);
  }

  for (const remote of remoteProfiles) {
    const remoteDetail = detailFromRemote(remote, current);
    remoteVoiceCache.set(remoteDetail.voiceName, remoteDetail);
    const localDetail = byName.get(remoteDetail.voiceName);
    if (localDetail) {
      byName.set(remoteDetail.voiceName, {
        ...localDetail,
        tags: [...new Set([...localDetail.tags, 'FunSpeech'])],
        syncStatus: localDetail.syncStatus === 'localOnly' ? 'synced' : localDetail.syncStatus,
      });
    } else {
      byName.set(remoteDetail.voiceName, remoteDetail);
    }
  }

  return [...byName.values()].map(detailToSummary);
}

export async function getVoiceDetail(voiceName: string): Promise<VoiceDetail> {
  const current = await currentVoiceName();
  try {
    const profile = await invoke<CustomVoiceProfile>('get_custom_voice', { voiceName });
    return detailFromProfile(profile, current);
  } catch (_error) {
    const cached = remoteVoiceCache.get(voiceName);
    if (cached) {
      return { ...cached, isCurrent: current === voiceName };
    }
    const remote = (await remoteVoices()).find((voice) => voice.voiceName === voiceName);
    if (remote) {
      return detailFromRemote(remote, current);
    }
    throw new Error(`音色不存在：${voiceName}`);
  }
}

async function saveProfile(
  detail: VoiceDetail,
  upload: WavUploadPayload | null
): Promise<CustomVoiceProfile> {
  return invoke<CustomVoiceProfile>('save_custom_voice_profile', {
    request: {
      voiceName: detail.voiceName,
      voiceInstruction: detail.voiceInstruction ?? '',
      referenceText: detail.referenceText,
      referenceAudioPath: upload ? null : (detail.referenceAudioPath ?? null),
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
    isCurrent: false,
    voiceInstruction: request.voiceInstruction ?? '',
    referenceText: request.referenceText,
    referenceAudioFileName: request.upload.fileName,
    editable: true,
  };
  const saved = await saveProfile(detail, request.upload);
  const report = await syncVoice(saved.voiceName, 'register');
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
  return {
    voiceName: saved.voiceName,
    message: report.message,
    updatedAt: saved.lastSyncedAt ?? saved.createdAt,
    syncStatus: syncStatusFromReport(report.syncStatus),
  };
}

export async function deleteVoice(voiceName: string): Promise<VoiceMutationResult> {
  const report = await invoke<VoiceSyncReport>('delete_custom_voice_sync', { voiceName });
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
  return {
    mode: request.mode,
    syncedCount: report.syncStatus === 'failed' ? 0 : report.localVoiceCount,
    failedCount: report.syncStatus === 'failed' ? 1 : 0,
    message: report.message,
  };
}

export async function setCurrentVoiceName(voiceName: string): Promise<void> {
  const settings = await getSettings();
  await updateSettings({
    ...settings,
    runtime: {
      ...settings.runtime,
      defaultVoiceName: voiceName,
    },
  });
}

export interface VoicePreviewState {
  playingVoiceName: string | null;
}

export async function toggleVoicePreview(detail: VoiceDetail): Promise<VoicePreviewState> {
  if (!detail.referenceAudioPath || detail.source === 'remote') {
    throw new Error('当前音色没有可试听的本地 wav 文件');
  }
  return invoke<VoicePreviewState>('toggle_voice_preview', {
    request: {
      voiceName: detail.voiceName,
      referenceAudioPath: detail.referenceAudioPath,
    },
  });
}

export async function stopVoicePreview(): Promise<VoicePreviewState> {
  return invoke<VoicePreviewState>('stop_voice_preview');
}
