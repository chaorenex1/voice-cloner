import { computed, reactive } from 'vue';
import {
  getVoiceDetail,
  listVoices,
  saveVoiceDetail,
  syncVoices,
} from '../services/tauri/voice-library';
import type {
  VoiceDetail,
  VoiceMutationResult,
  VoiceSummary,
  VoiceSyncMode,
  VoiceSyncResult,
} from '../utils/types/voice';

export interface VoiceLibraryState {
  voices: VoiceSummary[];
  selectedVoiceName: string | null;
  detail: VoiceDetail | null;
  search: string;
  loading: boolean;
  saving: boolean;
  lastMessage: string;
}

export interface CreateVoiceDraft {
  displayName: string;
  referenceText: string;
  referenceAudioPath: string;
}

const state = reactive<VoiceLibraryState>({
  voices: [],
  selectedVoiceName: null,
  detail: null,
  search: '',
  loading: false,
  saving: false,
  lastMessage: '音色库等待加载',
});

function voiceMatchesSearch(voice: VoiceSummary, search: string): boolean {
  const normalizedSearch = search.trim().toLowerCase();

  if (!normalizedSearch) {
    return true;
  }

  return [voice.voiceName, voice.displayName, voice.referenceTextPreview, ...voice.tags]
    .join(' ')
    .toLowerCase()
    .includes(normalizedSearch);
}

function voiceNameFromDisplayName(displayName: string): string {
  return (
    displayName
      .trim()
      .toLowerCase()
      .replace(/[^\da-z]+/g, '_')
      .replace(/^_+|_+$/g, '') || `custom_voice_${Date.now()}`
  );
}

function summaryFromDetail(detail: VoiceDetail): VoiceSummary {
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

export function useVoiceLibraryStore() {
  const filteredVoices = computed(() =>
    state.voices.filter((voice) => voiceMatchesSearch(voice, state.search))
  );

  const selectedVoice = computed(() =>
    state.voices.find((voice) => voice.voiceName === state.selectedVoiceName)
  );

  async function loadVoices(): Promise<void> {
    state.loading = true;
    try {
      state.voices = await listVoices();
      state.selectedVoiceName ??=
        state.voices.find((voice) => voice.isCurrent)?.voiceName ??
        state.voices[0]?.voiceName ??
        null;

      if (state.selectedVoiceName) {
        state.detail = await getVoiceDetail(state.selectedVoiceName);
      }

      state.lastMessage = `已加载 ${state.voices.length} 个音色`;
    } finally {
      state.loading = false;
    }
  }

  async function selectVoice(voiceName: string): Promise<void> {
    state.selectedVoiceName = voiceName;
    state.detail = await getVoiceDetail(voiceName);
    state.lastMessage = `${state.detail.displayName} 已载入详情`;
  }

  function updateDetail(patch: Partial<VoiceDetail>): void {
    if (!state.detail) {
      return;
    }

    state.detail = { ...state.detail, ...patch };
  }

  async function saveSelectedVoice(): Promise<VoiceMutationResult | null> {
    if (!state.detail) {
      return null;
    }

    state.saving = true;
    try {
      const result = await saveVoiceDetail(state.detail);
      state.lastMessage = result.message;
      updateDetail({ updatedAt: result.updatedAt, syncStatus: 'localOnly' });
      state.voices = state.voices.map((voice) =>
        voice.voiceName === result.voiceName
          ? { ...voice, updatedAt: result.updatedAt, syncStatus: 'localOnly' }
          : voice
      );
      return result;
    } finally {
      state.saving = false;
    }
  }

  function setCurrentVoice(voiceName: string): void {
    state.voices = state.voices.map((voice) => ({
      ...voice,
      isCurrent: voice.voiceName === voiceName,
    }));

    if (state.detail) {
      updateDetail({ isCurrent: state.detail.voiceName === voiceName });
    }

    state.lastMessage = '当前音色已切换';
  }

  function createLocalVoice(draft: CreateVoiceDraft): VoiceDetail {
    const now = new Date().toLocaleString('zh-CN', { hour12: false });
    const detail: VoiceDetail = {
      voiceName: voiceNameFromDisplayName(draft.displayName),
      displayName: draft.displayName,
      source: 'custom',
      tags: ['自定义', '本地草稿'],
      hasReferenceAudio: Boolean(draft.referenceAudioPath),
      updatedAt: now,
      referenceTextPreview: draft.referenceText.slice(0, 42),
      syncStatus: 'localOnly',
      isCurrent: false,
      voiceInstruction: '待补充音色指令',
      referenceText: draft.referenceText,
      referenceAudioPath: draft.referenceAudioPath || undefined,
      editable: true,
    };

    state.voices = [summaryFromDetail(detail), ...state.voices];
    state.selectedVoiceName = detail.voiceName;
    state.detail = detail;
    state.lastMessage = `${detail.displayName} 已创建为本地草稿`;

    return detail;
  }

  async function runSync(mode: VoiceSyncMode): Promise<VoiceSyncResult> {
    const result = await syncVoices({
      mode,
      voiceNames: state.selectedVoiceName ? [state.selectedVoiceName] : undefined,
    });
    state.lastMessage = result.message;
    return result;
  }

  return {
    state,
    filteredVoices,
    selectedVoice,
    loadVoices,
    selectVoice,
    updateDetail,
    saveSelectedVoice,
    setCurrentVoice,
    createLocalVoice,
    runSync,
  };
}
