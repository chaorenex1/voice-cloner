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
import {
  logRealtimeDebug,
  logRealtimeError,
  summarizeRealtimeSession,
  summarizeRealtimeSnapshot,
} from '../utils/realtime-debug';
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
    voiceName: '',
    displayName: '',
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
    logRealtimeDebug('store:load:start');
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
      logRealtimeDebug('store:load:success', {
        voiceCount: state.voices.length,
        selectedVoiceName: state.selectedVoiceName,
        inputDeviceId: settings.device.inputDeviceId,
        outputDeviceId: settings.device.outputDeviceId,
        virtualMicEnabled: settings.device.virtualMicEnabled,
        virtualMicDeviceId: settings.device.virtualMicDeviceId,
        defaultSampleRate: settings.runtime.defaultSampleRate,
        audioFrameMs: settings.runtime.audioFrameMs,
      });
    } catch (error) {
      state.lastError = error instanceof Error ? error.message : String(error);
      state.lastMessage = '实时链路加载失败';
      logRealtimeError('store:load:error', error);
    } finally {
      state.loading = false;
    }
  }

  async function start(): Promise<void> {
    if (!state.selectedVoiceName) {
      state.lastError = '请选择音色后再开始实时变声';
      logRealtimeDebug('store:start:blocked-missing-voice');
      return;
    }
    state.busy = true;
    state.lastError = null;
    logRealtimeDebug('store:start:begin', {
      voiceName: state.selectedVoiceName,
      runtimeParams: runtimeParams(),
    });
    try {
      const created = await createRealtimeSession({
        voiceName: state.selectedVoiceName,
        runtimeParams: runtimeParams(),
      });
      state.session = created;
      state.lastMessage = '正在连接 FunSpeech Realtime Voice...';
      logRealtimeDebug('store:start:session-created', summarizeRealtimeSession(created));
      state.session = await startRealtimeSession(created);
      logRealtimeDebug('store:start:session-running', summarizeRealtimeSession(state.session));
      state.snapshot = await getRealtimeStreamSnapshot(state.session);
      logRealtimeDebug('store:start:snapshot-ready', summarizeRealtimeSnapshot(state.snapshot));
      state.lastMessage = '实时透传闭环运行中';
    } catch (error) {
      state.lastError = error instanceof Error ? error.message : String(error);
      state.lastMessage = '实时链路启动失败';
      logRealtimeError('store:start:error', error, {
        selectedVoiceName: state.selectedVoiceName,
        session: state.session ? summarizeRealtimeSession(state.session) : null,
      });
    } finally {
      state.busy = false;
      logRealtimeDebug('store:start:end', {
        busy: state.busy,
        lastMessage: state.lastMessage,
        lastError: state.lastError,
      });
    }
  }

  async function stop(): Promise<void> {
    if (!state.session) {
      logRealtimeDebug('store:stop:skipped-no-session');
      return;
    }
    state.busy = true;
    logRealtimeDebug('store:stop:begin', summarizeRealtimeSession(state.session));
    try {
      state.session = await stopRealtimeSession(state.session);
      state.snapshot = null;
      state.lastMessage = '实时链路已停止';
      logRealtimeDebug('store:stop:success', summarizeRealtimeSession(state.session));
    } catch (error) {
      state.lastError = error instanceof Error ? error.message : String(error);
      logRealtimeError('store:stop:error', error, {
        session: state.session ? summarizeRealtimeSession(state.session) : null,
      });
    } finally {
      state.busy = false;
    }
  }

  async function selectVoice(voiceName: string): Promise<void> {
    const previousVoice = state.selectedVoiceName;
    state.selectedVoiceName = voiceName;
    logRealtimeDebug('store:select-voice:begin', {
      previousVoice,
      voiceName,
      running: isRunning.value,
      sessionId: state.session?.sessionId ?? null,
    });
    if (!state.session || !isRunning.value) {
      state.lastMessage = `${voiceName} 已选为目标音色`;
      logRealtimeDebug('store:select-voice:local-only', {
        selectedVoiceName: state.selectedVoiceName,
      });
      return;
    }
    state.busy = true;
    try {
      state.session = await switchRealtimeVoice(state.session.sessionId, voiceName);
      await refreshSnapshot();
      state.lastMessage = `FunSpeech 已切换到 ${voiceName}`;
      logRealtimeDebug('store:select-voice:success', {
        session: summarizeRealtimeSession(state.session),
        snapshot: state.snapshot ? summarizeRealtimeSnapshot(state.snapshot) : null,
      });
    } catch (error) {
      state.selectedVoiceName = previousVoice;
      state.lastError = error instanceof Error ? error.message : String(error);
      logRealtimeError('store:select-voice:error', error, {
        previousVoice,
        voiceName,
        session: state.session ? summarizeRealtimeSession(state.session) : null,
      });
    } finally {
      state.busy = false;
    }
  }

  async function setParam(key: keyof RealtimeParamState, value: number): Promise<void> {
    state.params[key] = value;
    logRealtimeDebug('store:set-param:local', {
      key,
      value,
      params: runtimeParams(),
      running: isRunning.value,
      sessionId: state.session?.sessionId ?? null,
    });
    if (!state.session || !isRunning.value) {
      return;
    }
    try {
      state.session = await updateRealtimeParams(state.session.sessionId, runtimeParams());
      state.lastMessage = '实时参数已发送到 FunSpeech';
      logRealtimeDebug('store:set-param:sent', summarizeRealtimeSession(state.session));
    } catch (error) {
      state.lastError = error instanceof Error ? error.message : String(error);
      logRealtimeError('store:set-param:error', error, {
        key,
        value,
        session: state.session ? summarizeRealtimeSession(state.session) : null,
      });
    }
  }

  async function refreshSnapshot(): Promise<void> {
    if (!state.session || !isRunning.value) {
      return;
    }
    try {
      state.snapshot = await getRealtimeStreamSnapshot(state.session);
      logRealtimeDebug('store:refresh-snapshot:success', summarizeRealtimeSnapshot(state.snapshot));
      if (state.snapshot.lastError) {
        state.lastError = state.snapshot.lastError;
        logRealtimeDebug('store:refresh-snapshot:last-error', {
          lastError: state.snapshot.lastError,
        });
      }
    } catch (error) {
      state.lastError = error instanceof Error ? error.message : String(error);
      logRealtimeError('store:refresh-snapshot:error', error, {
        session: summarizeRealtimeSession(state.session),
      });
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
