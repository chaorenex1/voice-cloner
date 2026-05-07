import { computed, reactive } from 'vue';
import {
  createCustomVoice,
  deleteVoice,
  getVoiceDetail,
  listVoices,
  saveVoiceDetail,
  setCurrentVoiceName,
  stopVoicePreview,
  syncVoices,
  toggleVoicePreview,
  type CreateCustomVoiceRequest,
  type WavUploadPayload,
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
  pendingReferenceAudio: WavUploadPayload | null;
  search: string;
  loading: boolean;
  saving: boolean;
  playingVoiceName: string | null;
  lastMessage: string;
}

export interface CreateVoiceDraft {
  displayName: string;
  referenceText: string;
  voiceInstruction?: string;
  upload: WavUploadPayload;
}

const state = reactive<VoiceLibraryState>({
  voices: [],
  selectedVoiceName: null,
  detail: null,
  pendingReferenceAudio: null,
  search: '',
  loading: false,
  saving: false,
  playingVoiceName: null,
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

function patchVoiceSummary(result: VoiceMutationResult): void {
  state.voices = state.voices.map((voice) =>
    voice.voiceName === result.voiceName
      ? { ...voice, updatedAt: result.updatedAt, syncStatus: result.syncStatus }
      : voice
  );
}

export function useVoiceLibraryStore() {
  const filteredVoices = computed(() =>
    state.voices.filter((voice) => voiceMatchesSearch(voice, state.search))
  );

  const selectedVoice = computed(() =>
    state.voices.find((voice) => voice.voiceName === state.selectedVoiceName)
  );

  function setSearch(value: string): void {
    state.search = value;
    const matchedCount = filteredVoices.value.length;
    state.lastMessage = value.trim()
      ? `搜索到 ${matchedCount} 个匹配音色`
      : `已加载 ${state.voices.length} 个音色`;
  }

  async function loadVoices(): Promise<void> {
    state.loading = true;
    try {
      const previousSelection = state.selectedVoiceName;
      state.voices = await listVoices();
      state.selectedVoiceName =
        (previousSelection && state.voices.some((voice) => voice.voiceName === previousSelection)
          ? previousSelection
          : null) ??
        state.voices.find((voice) => voice.isCurrent)?.voiceName ??
        state.voices[0]?.voiceName ??
        null;

      state.detail = state.selectedVoiceName ? await getVoiceDetail(state.selectedVoiceName) : null;
      state.pendingReferenceAudio = null;
      state.lastMessage = `已加载 ${state.voices.length} 个音色`;
    } finally {
      state.loading = false;
    }
  }

  async function selectVoice(voiceName: string): Promise<void> {
    if (state.playingVoiceName && state.playingVoiceName !== voiceName) {
      const stopped = await stopVoicePreview();
      state.playingVoiceName = stopped.playingVoiceName;
    }
    state.selectedVoiceName = voiceName;
    state.detail = await getVoiceDetail(voiceName);
    state.pendingReferenceAudio = null;
    state.lastMessage = `${state.detail.displayName} 已载入详情`;
  }

  function updateDetail(patch: Partial<VoiceDetail>): void {
    if (!state.detail) {
      return;
    }

    state.detail = { ...state.detail, ...patch };
  }

  function attachReferenceAudio(upload: WavUploadPayload): void {
    state.pendingReferenceAudio = upload;
    updateDetail({
      hasReferenceAudio: true,
      referenceAudioFileName: upload.fileName,
      referenceAudioPath: upload.fileName,
    });
    state.lastMessage = `已选择 wav 参考音频：${upload.fileName}`;
  }

  async function saveSelectedVoice(): Promise<VoiceMutationResult | null> {
    if (!state.detail) {
      return null;
    }

    state.saving = true;
    try {
      const result = await saveVoiceDetail(state.detail, state.pendingReferenceAudio);
      state.lastMessage = result.message;
      updateDetail({ updatedAt: result.updatedAt, syncStatus: result.syncStatus });
      patchVoiceSummary(result);
      state.pendingReferenceAudio = null;
      await loadVoices();
      return result;
    } finally {
      state.saving = false;
    }
  }

  async function createLocalVoice(draft: CreateVoiceDraft): Promise<VoiceMutationResult> {
    state.saving = true;
    try {
      const result = await createCustomVoice(draft as CreateCustomVoiceRequest);
      state.lastMessage = result.message;
      await loadVoices();
      state.selectedVoiceName = result.voiceName;
      state.detail = await getVoiceDetail(result.voiceName);
      return result;
    } finally {
      state.saving = false;
    }
  }

  async function removeSelectedVoice(): Promise<VoiceMutationResult | null> {
    if (!state.detail || !state.detail.editable) {
      return null;
    }
    const target = state.detail.voiceName;
    if (state.playingVoiceName === target) {
      const stopped = await stopVoicePreview();
      state.playingVoiceName = stopped.playingVoiceName;
    }
    state.saving = true;
    try {
      const result = await deleteVoice(target);
      state.lastMessage = result.message;
      state.voices = state.voices.filter((voice) => voice.voiceName !== target);
      state.selectedVoiceName = state.voices[0]?.voiceName ?? null;
      state.detail = state.selectedVoiceName ? await getVoiceDetail(state.selectedVoiceName) : null;
      return result;
    } finally {
      state.saving = false;
    }
  }

  async function setCurrentVoice(voiceName: string): Promise<void> {
    await setCurrentVoiceName(voiceName);
    state.voices = state.voices.map((voice) => ({
      ...voice,
      isCurrent: voice.voiceName === voiceName,
    }));

    if (state.detail) {
      updateDetail({ isCurrent: state.detail.voiceName === voiceName });
    }

    state.lastMessage = '当前音色已保存到本地设置';
  }

  async function previewVoice(voiceName?: string): Promise<void> {
    const target = voiceName ?? state.detail?.voiceName;
    if (!target) {
      state.lastMessage = '请选择要试听的音色';
      return;
    }
    const detail = state.detail?.voiceName === target ? state.detail : await getVoiceDetail(target);
    const playback = await toggleVoicePreview(detail);
    state.playingVoiceName = playback.playingVoiceName;
    state.lastMessage = playback.playingVoiceName ? `${target} 正在试听` : `${target} 已停止试听`;
  }

  async function runSync(mode: VoiceSyncMode): Promise<VoiceSyncResult> {
    const result = await syncVoices({
      mode,
      voiceNames: state.selectedVoiceName ? [state.selectedVoiceName] : undefined,
    });
    state.lastMessage = result.message;
    await loadVoices();
    return result;
  }

  return {
    state,
    filteredVoices,
    selectedVoice,
    setSearch,
    loadVoices,
    selectVoice,
    updateDetail,
    attachReferenceAudio,
    saveSelectedVoice,
    setCurrentVoice,
    createLocalVoice,
    removeSelectedVoice,
    previewVoice,
    runSync,
  };
}
