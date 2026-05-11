import { computed, reactive } from 'vue';
import {
  createRealtimeSession,
  getRealtimeStreamSnapshot,
  startRealtimeInput,
  startRealtimeFileInput,
  startRealtimeMonitor,
  startRealtimeSession,
  stopRealtimeInput,
  stopRealtimeMonitor,
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
import type {
  RealtimeSession,
  RealtimeStreamSnapshot,
  RuntimeParams,
} from '../utils/types/realtime';
import type { AppSettings } from '../utils/types/settings';
import type { VoiceSummary } from '../utils/types/voice';
import type { DenoiseMode, VoicePostProcessConfig } from '../utils/types/voice-separation';
import { defaultStereoPostProcessConfig } from '../utils/types/voice-separation';

export interface RealtimeParamState {
  pitchRate: number;
  speechRate: number;
  volume: number;
}

export interface RealtimeState {
  voices: VoiceSummary[];
  settings: AppSettings | null;
  selectedVoiceName: string | null;
  inputSource: 'microphone' | 'localFile';
  selectedInputFile: File | null;
  params: RealtimeParamState;
  postProcessConfig: VoicePostProcessConfig;
  session: RealtimeSession | null;
  snapshot: RealtimeStreamSnapshot | null;
  inputCapturing: boolean;
  monitoring: boolean;
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
  },
];

const state = reactive<RealtimeState>({
  voices: [],
  settings: null,
  selectedVoiceName: null,
  inputSource: 'microphone',
  selectedInputFile: null,
  params: {
    pitchRate: 0,
    speechRate: 0,
    volume: 50,
  },
  postProcessConfig: { ...defaultStereoPostProcessConfig },
  session: null,
  snapshot: null,
  inputCapturing: false,
  monitoring: false,
  loading: false,
  busy: false,
  lastMessage: '实时链路等待加载',
  lastError: null,
});

const LAST_REALTIME_VOICE_STORAGE_KEY = 'voice-cloner:last-realtime-voice-name';
const SWITCH_CONFIRM_TIMEOUT_MS = 5000;
const SWITCH_CONFIRM_POLL_MS = 150;

function runtimeParams(): RuntimeParams {
  return {
    values: {
      pitchRate: state.params.pitchRate,
      speechRate: state.params.speechRate,
      volume: state.params.volume,
      prompt: '',
      emotionControl: 'off',
    },
  };
}

function isRunningStatus(status: string | null | undefined): boolean {
  return status === 'running' || status === 'connecting';
}

function hasVoice(
  voices: VoiceSummary[],
  voiceName: string | null | undefined
): voiceName is string {
  return Boolean(voiceName && voices.some((voice) => voice.voiceName === voiceName));
}

function isRealtimeVoiceSelectable(voice: VoiceSummary): boolean {
  return voice.source === 'remote' || voice.source === 'preset' || voice.syncStatus === 'synced';
}

function selectableRealtimeVoices(voices: VoiceSummary[]): VoiceSummary[] {
  return voices.filter(isRealtimeVoiceSelectable);
}

function lastRealtimeVoiceName(): string | null {
  if (typeof window === 'undefined') {
    return null;
  }
  return window.localStorage.getItem(LAST_REALTIME_VOICE_STORAGE_KEY);
}

function saveLastRealtimeVoiceName(voiceName: string): void {
  if (typeof window === 'undefined') {
    return;
  }
  window.localStorage.setItem(LAST_REALTIME_VOICE_STORAGE_KEY, voiceName);
}

function clearLastRealtimeVoiceName(): void {
  if (typeof window === 'undefined') {
    return;
  }
  window.localStorage.removeItem(LAST_REALTIME_VOICE_STORAGE_KEY);
}

function resolveSelectedVoiceName(voices: VoiceSummary[]): string | null {
  const availableVoices = selectableRealtimeVoices(voices);
  if (hasVoice(availableVoices, state.selectedVoiceName)) {
    return state.selectedVoiceName;
  }

  const lastVoiceName = lastRealtimeVoiceName();
  if (hasVoice(availableVoices, lastVoiceName)) {
    return lastVoiceName;
  }

  return availableVoices[0]?.voiceName ?? null;
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => {
    window.setTimeout(resolve, ms);
  });
}

export function useRealtimeStore() {
  const selectedVoice = computed(() =>
    state.voices.find((voice) => voice.voiceName === state.selectedVoiceName)
  );
  const selectedVoiceIsSelectable = computed(() =>
    selectedVoice.value ? isRealtimeVoiceSelectable(selectedVoice.value) : false
  );

  const isRunning = computed(() => isRunningStatus(state.session?.status));
  const isRealtimeDebugEnabled = computed(() =>
    Boolean(state.settings?.runtime.realtimeDebugEnabled)
  );
  const isInputCapturing = computed(() =>
    isRealtimeDebugEnabled.value
      ? ['capturing', 'starting'].includes(state.snapshot?.inputState ?? '')
      : state.inputCapturing
  );
  const isMonitoring = computed(() =>
    isRealtimeDebugEnabled.value
      ? ['listening', 'starting'].includes(state.snapshot?.monitorState ?? '')
      : state.monitoring
  );
  const canControlStream = computed(() => Boolean(state.session) && isRunning.value && !state.busy);

  const canStart = computed(
    () => Boolean(state.selectedVoiceName) && selectedVoiceIsSelectable.value && !state.busy && !isRunning.value
  );

  async function waitForConfiguredVoice(
    session: RealtimeSession,
    voiceName: string
  ): Promise<RealtimeStreamSnapshot> {
    const deadline = Date.now() + SWITCH_CONFIRM_TIMEOUT_MS;
    let latestSnapshot: RealtimeStreamSnapshot | null = null;
    while (Date.now() <= deadline) {
      latestSnapshot = await getRealtimeStreamSnapshot(session);
      if (latestSnapshot.lastError) {
        throw new Error(latestSnapshot.lastError);
      }
      if (latestSnapshot.configuredVoiceName === voiceName) {
        return latestSnapshot;
      }
      await sleep(SWITCH_CONFIRM_POLL_MS);
    }
    const configuredVoice = latestSnapshot?.configuredVoiceName || 'unknown';
    throw new Error(`FunSpeech 未确认切换到 ${voiceName}，当前服务端音色为 ${configuredVoice}`);
  }

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
      state.selectedVoiceName = resolveSelectedVoiceName(state.voices);
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
        realtimeDebugEnabled: settings.runtime.realtimeDebugEnabled,
        realtimePlaybackAckEnabled: settings.runtime.realtimePlaybackAckEnabled,
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
        postProcessConfig: { ...state.postProcessConfig, channels: 'stereo', trimSilence: false },
      });
      state.session = created;
      state.lastMessage = '正在连接 FunSpeech Realtime Voice...';
      logRealtimeDebug('store:start:session-created', summarizeRealtimeSession(created));
      state.session = await startRealtimeSession(created);
      logRealtimeDebug('store:start:session-running', summarizeRealtimeSession(state.session));
      if (isRealtimeDebugEnabled.value) {
        state.snapshot = await getRealtimeStreamSnapshot(state.session);
        logRealtimeDebug('store:start:snapshot-ready', summarizeRealtimeSnapshot(state.snapshot));
        state.lastMessage = state.snapshot.lastPrompt ?? '实时会话已连接，点击麦克风开始采集输入音频';
      } else {
        state.snapshot = null;
        state.lastMessage = '实时会话已连接，点击麦克风开始采集输入音频';
      }
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
      state.inputCapturing = false;
      state.monitoring = false;
      state.lastMessage = '实时会话、麦克风输入和监听输出已停止';
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

  async function toggleInput(): Promise<void> {
    if (!state.session || !isRunning.value) {
      state.lastMessage = '请先点击开始建立 FunSpeech 实时会话';
      return;
    }
    state.busy = true;
    state.lastError = null;
    const wasCapturing = isInputCapturing.value;
    try {
      if (wasCapturing) {
        const snapshot = await stopRealtimeInput(state.session);
        state.snapshot = isRealtimeDebugEnabled.value ? snapshot : null;
        state.inputCapturing = false;
      } else if (state.inputSource === 'localFile') {
        if (!state.selectedInputFile) {
          state.lastError = '请选择本地 WAV 音频后再开始模拟';
          state.busy = false;
          return;
        }
        const audioBytes = Array.from(new Uint8Array(await state.selectedInputFile.arrayBuffer()));
        const snapshot = await startRealtimeFileInput(state.session, {
          fileName: state.selectedInputFile.name,
          audioBytes,
        });
        state.snapshot = isRealtimeDebugEnabled.value ? snapshot : null;
        state.inputCapturing = true;
      } else {
        const snapshot = await startRealtimeInput(state.session);
        state.snapshot = isRealtimeDebugEnabled.value ? snapshot : null;
        state.inputCapturing = true;
      }
      state.lastMessage = wasCapturing
        ? '麦克风输入已关闭，会话保持连接'
        : state.inputSource === 'localFile'
          ? '正在用本地音频模拟实时输入'
          : '麦克风正在采集输入音频';
      logRealtimeDebug('store:toggle-input:success', {
        action: wasCapturing ? 'stop' : 'start',
        snapshot: state.snapshot ? summarizeRealtimeSnapshot(state.snapshot) : null,
      });
    } catch (error) {
      state.lastError = error instanceof Error ? error.message : String(error);
      state.lastMessage = '麦克风输入控制失败';
      logRealtimeError('store:toggle-input:error', error, {
        session: state.session ? summarizeRealtimeSession(state.session) : null,
      });
    } finally {
      state.busy = false;
    }
  }

  function setInputSource(inputSource: 'microphone' | 'localFile'): void {
    state.inputSource = inputSource;
    state.lastMessage = inputSource === 'localFile' ? '已切换到本地音频模拟' : '已切换到麦克风输入';
  }

  function setSelectedInputFile(file: File | null): void {
    state.selectedInputFile = file;
    if (file) {
      state.lastMessage = `已选择实时模拟音频: ${file.name}`;
    }
  }

  async function toggleMonitor(): Promise<void> {
    if (!state.session || !isRunning.value) {
      state.lastMessage = '请先点击开始建立 FunSpeech 实时会话';
      return;
    }
    state.busy = true;
    state.lastError = null;
    const wasMonitoring = isMonitoring.value;
    try {
      const snapshot = wasMonitoring
        ? await stopRealtimeMonitor(state.session)
        : await startRealtimeMonitor(state.session);
      state.snapshot = isRealtimeDebugEnabled.value ? snapshot : null;
      state.monitoring = !wasMonitoring;
      state.lastMessage = wasMonitoring ? '监听输出已停止' : '正在通过监听输出设备播放转换后语音';
      logRealtimeDebug('store:toggle-monitor:success', {
        action: wasMonitoring ? 'stop' : 'start',
        snapshot: state.snapshot ? summarizeRealtimeSnapshot(state.snapshot) : null,
      });
    } catch (error) {
      state.lastError = error instanceof Error ? error.message : String(error);
      state.lastMessage = '监听输出控制失败';
      logRealtimeError('store:toggle-monitor:error', error, {
        session: state.session ? summarizeRealtimeSession(state.session) : null,
      });
    } finally {
      state.busy = false;
    }
  }

  async function selectVoice(voiceName: string): Promise<void> {
    const previousVoice = state.selectedVoiceName;
    const voice = state.voices.find((candidate) => candidate.voiceName === voiceName);
    if (voice && !isRealtimeVoiceSelectable(voice)) {
      state.lastError = `${voice.displayName || voice.voiceName} 尚未同步到 FunSpeech，不能用于实时变声`;
      state.lastMessage = '请先在音色库同步该音色后再切换实时变声';
      logRealtimeDebug('store:select-voice:blocked-unsynced', {
        voiceName,
        syncStatus: voice.syncStatus,
        source: voice.source,
      });
      return;
    }
    state.selectedVoiceName = voiceName;
    saveLastRealtimeVoiceName(voiceName);
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
    state.lastError = null;
    state.lastMessage = `正在通知 FunSpeech 切换到 ${voiceName}`;
    try {
      state.session = await switchRealtimeVoice(state.session.sessionId, voiceName);
      state.snapshot = await waitForConfiguredVoice(state.session, voiceName);
      state.lastMessage = `FunSpeech 已确认切换到 ${voiceName}`;
      logRealtimeDebug('store:select-voice:success', {
        session: summarizeRealtimeSession(state.session),
        snapshot: state.snapshot ? summarizeRealtimeSnapshot(state.snapshot) : null,
      });
    } catch (error) {
      state.selectedVoiceName = previousVoice;
      if (previousVoice) {
        saveLastRealtimeVoiceName(previousVoice);
      } else {
        clearLastRealtimeVoiceName();
      }
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

  function setPostProcessDenoise(denoiseMode: DenoiseMode): void {
    state.postProcessConfig = {
      ...state.postProcessConfig,
      denoiseMode,
      channels: 'stereo',
      trimSilence: false,
    };
  }

  function setPostProcessLufs(targetLufs: number): void {
    state.postProcessConfig = {
      ...state.postProcessConfig,
      targetLufs,
      loudnessNormalization: true,
      channels: 'stereo',
      trimSilence: false,
    };
  }

  async function refreshSnapshot(): Promise<void> {
    if (!state.session || !isRunning.value || !isRealtimeDebugEnabled.value) {
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
      } else if (state.snapshot.lastPrompt) {
        state.lastMessage = state.snapshot.lastPrompt;
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
    selectedVoiceIsSelectable,
    isRunning,
    isRealtimeDebugEnabled,
    isInputCapturing,
    isMonitoring,
    canControlStream,
    canStart,
    isRealtimeVoiceSelectable,
    load,
    start,
    stop,
    toggleInput,
    setInputSource,
    setSelectedInputFile,
    toggleMonitor,
    selectVoice,
    setParam,
    setPostProcessDenoise,
    setPostProcessLufs,
    refreshSnapshot,
  };
}
