import { computed, reactive } from 'vue';
import {
  createRealtimeSession,
  getRealtimeStreamSnapshot,
  startRealtimeSession,
  stopRealtimeSession,
  switchRealtimeVoice,
  updateRealtimeParams,
} from '../services/tauri/realtime';
import { getSettings } from '../services/tauri/settings';
import { listVoices } from '../services/tauri/voice-library';
import type { RealtimeSession, RealtimeStreamSnapshot, RuntimeParams } from '../utils/types/realtime';
import type { AppSettings } from '../utils/types/settings';
import type { VoiceSummary } from '../utils/types/voice';

export interface RealtimeParamState {
  pitch: number;
  strength: number;
  brightness: number;
}

export interface RealtimeState {
  voices: VoiceSummary[];
  settings: AppSettings | null;
  selectedVoiceName: string | null;
  params: RealtimeParamState;
  session: RealtimeSession | null;
  snapshot: RealtimeStreamSnapshot | null;
  loading: boolean;
  busy: boolean;
  lastMessage: string;
  lastError: string | null;
}

const demoVoices: VoiceSummary[] = [
  {
    voiceName: 'desktop_voice',
    displayName: 'desktop_voice',
    source: 'remote',
    tags: ['FunSpeech', '预览'],
    hasReferenceAudio: true,
    updatedAt: 'preview',
    referenceTextPreview: '前端预览音色',
    syncStatus: 'synced',
    isCurrent: true,
  },
];

const state = reactive<RealtimeState>({
  voices: [],
  settings: null,
  selectedVoiceName: null,
  params: {
    pitch: 1,
    strength: 1,
    brightness: 1,
  },
  session: null,
  snapshot: null,
  loading: false,
  busy: false,
  lastMessage: '实时链路等待加载',
  lastError: null,
});

function runtimeParams(): RuntimeParams {
  return {
    values: {
      pitch: state.params.pitch,
      strength: state.params.strength,
      brightness: state.params.brightness,
    },
  };
}

function isRunningStatus(status: string | null | undefined): boolean {
  return status === 'running' || status === 'connecting';
}

export function useRealtimeStore() {
  const selectedVoice = computed(() =>
    state.voices.find((voice) => voice.voiceName === state.selectedVoiceName)
  );

  const isRunning = computed(() => isRunningStatus(state.session?.status));

  const canStart = computed(
    () => Boolean(state.selectedVoiceName) && !state.busy && !isRunning.value
  );

  async function load(): Promise<void> {
    state.loading = true;
    state.lastError = null;
    try {
      const [settings, voices] = await Promise.all([
        getSettings(),
        listVoices().catch(() => demoVoices),
      ]);
      state.settings = settings;
      state.voices = voices.length > 0 ? voices : demoVoices;
      state.selectedVoiceName =
        state.selectedVoiceName ??
        settings.runtime.defaultVoiceName ??
        state.voices.find((voice) => voice.isCurrent)?.voiceName ??
        state.voices[0]?.voiceName ??
        null;
      state.lastMessage = `已加载 ${state.voices.length} 个可用音色`;
    } catch (error) {
      state.lastError = error instanceof Error ? error.message : String(error);
      state.lastMessage = '实时链路加载失败';
    } finally {
      state.loading = false;
    }
  }

  async function start(): Promise<void> {
    if (!state.selectedVoiceName) {
      state.lastError = '请选择音色后再开始实时变声';
      return;
    }
    state.busy = true;
    state.lastError = null;
    try {
      const created = await createRealtimeSession({
        voiceName: state.selectedVoiceName,
        runtimeParams: runtimeParams(),
      });
      state.session = created;
      state.lastMessage = '正在连接 FunSpeech Realtime Voice...';
      state.session = await startRealtimeSession(created);
      state.snapshot = await getRealtimeStreamSnapshot(state.session);
      state.lastMessage = '实时透传闭环运行中';
    } catch (error) {
      state.lastError = error instanceof Error ? error.message : String(error);
      state.lastMessage = '实时链路启动失败';
    } finally {
      state.busy = false;
    }
  }

  async function stop(): Promise<void> {
    if (!state.session) {
      return;
    }
    state.busy = true;
    try {
      state.session = await stopRealtimeSession(state.session);
      state.snapshot = null;
      state.lastMessage = '实时链路已停止';
    } catch (error) {
      state.lastError = error instanceof Error ? error.message : String(error);
    } finally {
      state.busy = false;
    }
  }

  async function selectVoice(voiceName: string): Promise<void> {
    const previousVoice = state.selectedVoiceName;
    state.selectedVoiceName = voiceName;
    if (!state.session || !isRunning.value) {
      state.lastMessage = `${voiceName} 已选为目标音色`;
      return;
    }
    state.busy = true;
    try {
      state.session = await switchRealtimeVoice(state.session.sessionId, voiceName);
      await refreshSnapshot();
      state.lastMessage = `FunSpeech 已切换到 ${voiceName}`;
    } catch (error) {
      state.selectedVoiceName = previousVoice;
      state.lastError = error instanceof Error ? error.message : String(error);
    } finally {
      state.busy = false;
    }
  }

  async function setParam(key: keyof RealtimeParamState, value: number): Promise<void> {
    state.params[key] = value;
    if (!state.session || !isRunning.value) {
      return;
    }
    try {
      state.session = await updateRealtimeParams(state.session.sessionId, runtimeParams());
      state.lastMessage = '实时参数已发送到 FunSpeech';
    } catch (error) {
      state.lastError = error instanceof Error ? error.message : String(error);
    }
  }

  async function refreshSnapshot(): Promise<void> {
    if (!state.session || !isRunning.value) {
      return;
    }
    try {
      state.snapshot = await getRealtimeStreamSnapshot(state.session);
      if (state.snapshot.lastError) {
        state.lastError = state.snapshot.lastError;
      }
    } catch (error) {
      state.lastError = error instanceof Error ? error.message : String(error);
    }
  }

  return {
    state,
    selectedVoice,
    isRunning,
    canStart,
    load,
    start,
    stop,
    selectVoice,
    setParam,
    refreshSnapshot,
  };
}
