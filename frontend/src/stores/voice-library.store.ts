import { computed, reactive } from 'vue';
import {
  createCustomVoice,
  deleteVoice,
  getVoiceDetail,
  listenVoicePreviewFinished,
  listVoices,
  saveVoiceDetail,
  stopVoicePreview,
  syncVoices,
  toggleVoicePreview,
  transcribeReferenceAudio,
  type CreateCustomVoiceRequest,
  type WavUploadPayload,
} from '../services/tauri/voice-library';
import type { VoicePostProcessConfig, VoiceSeparationModel } from '../utils/types/voice-separation';
import type {
  VoiceDetail,
  VoiceMutationResult,
  VoiceSummary,
  VoiceSyncMode,
  VoiceSyncResult,
} from '../utils/types/voice';

export type VoiceLibraryOperation =
  | 'loadingVoices'
  | 'syncingCloud'
  | 'refreshingCloud'
  | 'uploadingAudio'
  | 'recognizingAudio'
  | 'savingVoice'
  | 'creatingVoice'
  | 'deletingVoice'
  | null;

export interface VoiceLibraryState {
  voices: VoiceSummary[];
  selectedVoiceName: string | null;
  detail: VoiceDetail | null;
  pendingReferenceAudio: WavUploadPayload | null;
  search: string;
  loading: boolean;
  saving: boolean;
  operation: VoiceLibraryOperation;
  playingVoiceName: string | null;
  lastMessage: string;
}

export interface CreateVoiceDraft {
  displayName: string;
  referenceText: string;
  voiceInstruction?: string;
  upload: WavUploadPayload;
  skipSeparation?: boolean;
  separationModel?: VoiceSeparationModel;
  postProcessConfig?: VoicePostProcessConfig;
}

const state = reactive<VoiceLibraryState>({
  voices: [],
  selectedVoiceName: null,
  detail: null,
  pendingReferenceAudio: null,
  search: '',
  loading: false,
  saving: false,
  operation: null,
  playingVoiceName: null,
  lastMessage: '音色库等待加载',
});

let previewFinishListenerStarted = false;

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

function messageFromError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function startPreviewFinishListener(): void {
  if (previewFinishListenerStarted) {
    return;
  }

  previewFinishListenerStarted = true;
  void listenVoicePreviewFinished((event) => {
    if (state.playingVoiceName === event.voiceName) {
      state.playingVoiceName = event.playingVoiceName;
      state.lastMessage = `${event.voiceName} 试听已结束`;
    }
  }).catch((error) => {
    previewFinishListenerStarted = false;
    state.lastMessage = `试听结束事件监听失败：${messageFromError(error)}`;
  });
}

export function useVoiceLibraryStore() {
  startPreviewFinishListener();

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

  async function loadVoices(options: { preserveOperation?: boolean } = {}): Promise<void> {
    const shouldOwnOperation = !options.preserveOperation;
    if (shouldOwnOperation) {
      state.loading = true;
      state.operation = 'loadingVoices';
    }
    try {
      const previousSelection = state.selectedVoiceName;
      state.voices = await listVoices();
      state.selectedVoiceName =
        (previousSelection && state.voices.some((voice) => voice.voiceName === previousSelection)
          ? previousSelection
          : null) ??
        state.voices[0]?.voiceName ??
        null;

      state.detail = state.selectedVoiceName ? await getVoiceDetail(state.selectedVoiceName) : null;
      state.pendingReferenceAudio = null;
      state.lastMessage = `已加载 ${state.voices.length} 个音色`;
    } finally {
      if (shouldOwnOperation) {
        state.loading = false;
        state.operation = null;
      }
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
    });
    state.lastMessage = `已选择 wav 参考音频：${upload.fileName}`;
  }

  async function recognizeReferenceAudio(upload: WavUploadPayload): Promise<string | null> {
    state.operation = 'recognizingAudio';
    try {
      const result = await transcribeReferenceAudio(upload);
      state.lastMessage = `已自动识别参考文本：${result.text.slice(0, 42)}`;
      return result.text;
    } catch (error) {
      state.lastMessage = `自动识别参考文本失败：${messageFromError(error)}`;
      return null;
    } finally {
      state.operation = null;
    }
  }

  async function saveSelectedVoice(): Promise<VoiceMutationResult | null> {
    if (!state.detail) {
      return null;
    }

    state.saving = true;
    state.operation = 'savingVoice';
    try {
      const result = await saveVoiceDetail(state.detail, state.pendingReferenceAudio);
      state.lastMessage = result.message;
      updateDetail({ updatedAt: result.updatedAt, syncStatus: result.syncStatus });
      patchVoiceSummary(result);
      state.pendingReferenceAudio = null;
      await loadVoices({ preserveOperation: true });
      return result;
    } finally {
      state.saving = false;
      state.operation = null;
    }
  }

  async function createLocalVoice(draft: CreateVoiceDraft): Promise<VoiceMutationResult> {
    state.saving = true;
    state.operation = 'creatingVoice';
    try {
      const result = await createCustomVoice(draft as CreateCustomVoiceRequest);
      state.lastMessage = result.message;
      await loadVoices({ preserveOperation: true });
      state.selectedVoiceName = result.voiceName;
      state.detail = await getVoiceDetail(result.voiceName);
      return result;
    } finally {
      state.saving = false;
      state.operation = null;
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
    state.operation = 'deletingVoice';
    try {
      const result = await deleteVoice(target);
      state.lastMessage = result.message;
      state.voices = state.voices.filter((voice) => voice.voiceName !== target);
      state.selectedVoiceName = state.voices[0]?.voiceName ?? null;
      state.detail = state.selectedVoiceName ? await getVoiceDetail(state.selectedVoiceName) : null;
      return result;
    } finally {
      state.saving = false;
      state.operation = null;
    }
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
    state.loading = true;
    state.operation = mode === 'full' ? 'syncingCloud' : 'refreshingCloud';
    try {
      const result = await syncVoices({
        mode,
        voiceNames: state.selectedVoiceName ? [state.selectedVoiceName] : undefined,
      });
      state.lastMessage = result.message;
      await loadVoices({ preserveOperation: true });
      return result;
    } finally {
      state.loading = false;
      state.operation = null;
    }
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
    recognizeReferenceAudio,
    saveSelectedVoice,
    createLocalVoice,
    removeSelectedVoice,
    previewVoice,
    runSync,
  };
}
