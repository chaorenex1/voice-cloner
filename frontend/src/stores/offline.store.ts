import { computed, reactive } from 'vue';
import { open, save } from '@tauri-apps/plugin-dialog';
import {
  cancelOfflineJob,
  clearOfflineJobs,
  createOfflineAudioJob,
  createOfflineTextJob,
  deleteOfflineJob,
  downloadOfflineJob,
  listOfflineTtsEmotions,
  listenOfflineJobPreviewFinished,
  listenOfflineJobUpdated,
  listOfflineJobs,
  retryOfflineJob,
  startOfflineJob,
  stopOfflineJobPreview,
  toggleOfflineJobPreview,
} from '../services/tauri/offline';
import { listVoices } from '../services/tauri/voice-library';
import type { OfflineInputType, OfflineJob, RuntimeParams, TtsEmotionOption } from '../utils/types/offline';
import type { VoiceSummary } from '../utils/types/voice';
import type { DenoiseMode, VoicePostProcessConfig } from '../utils/types/voice-separation';
import { defaultStereoPostProcessConfig } from '../utils/types/voice-separation';

export interface OfflineParamState {
  pitchRate: number;
  speechRate: number;
  volume: number;
}

export interface OfflineState {
  inputType: OfflineInputType;
  text: string;
  selectedFile: File | null;
  selectedAudioPath: string | null;
  selectedAudioFileName: string | null;
  selectedVoiceName: string | null;
  selectedEmotionLabel: string | null;
  outputFormat: 'wav';
  params: OfflineParamState;
  postProcessConfig: VoicePostProcessConfig;
  emotionOptions: TtsEmotionOption[];
  voices: VoiceSummary[];
  jobs: OfflineJob[];
  currentJob: OfflineJob | null;
  playingJobId: string | null;
  loading: boolean;
  busy: boolean;
  lastMessage: string;
  lastError: string | null;
}

const demoVoices: VoiceSummary[] = [
  {
    voiceName: '中文女',
    displayName: '中文女',
    source: 'preset',
    tags: ['内置'],
    hasReferenceAudio: false,
    updatedAt: 'preview',
    referenceTextPreview: 'FunSpeech 内置音色',
    syncStatus: 'synced',
  },
];

const state = reactive<OfflineState>({
  inputType: 'audio',
  text: '',
  selectedFile: null,
  selectedAudioPath: null,
  selectedAudioFileName: null,
  selectedVoiceName: null,
  selectedEmotionLabel: null,
  outputFormat: 'wav',
  params: {
    pitchRate: 0,
    speechRate: 0,
    volume: 50,
  },
  postProcessConfig: { ...defaultStereoPostProcessConfig },
  emotionOptions: [],
  voices: [],
  jobs: [],
  currentJob: null,
  playingJobId: null,
  loading: false,
  busy: false,
  lastMessage: '离线工作台等待加载',
  lastError: null,
});

const MAX_AUDIO_BYTES = 50 * 1024 * 1024;
const MAX_TEXT_LENGTH = 1200;

function runtimeParams(): RuntimeParams {
  const selectedEmotion = state.emotionOptions.find(
    (emotion) => emotion.label === state.selectedEmotionLabel
  );
  return {
    values: {
      pitchRate: state.params.pitchRate,
      speechRate: state.params.speechRate,
      volume: state.params.volume,
      ...(selectedEmotion ? { prompt: selectedEmotion.prompt } : {}),
    },
  };
}

function messageFromError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function isSupportedAudioFile(file: File): boolean {
  return /\.wav$/i.test(file.name);
}

function fileNameFromPath(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}

function validateBeforeSubmit(): string | null {
  if (!state.selectedVoiceName) {
    return '请选择目标音色';
  }
  if (state.inputType === 'text') {
    const text = state.text.trim();
    if (!text) {
      return '请输入要转换的文本';
    }
    if (text.length > MAX_TEXT_LENGTH) {
      return `文本不能超过 ${MAX_TEXT_LENGTH} 字`;
    }
    return null;
  }

  const selectedFile = state.selectedFile;
  const selectedAudioPath = state.selectedAudioPath;
  if (!selectedFile && !selectedAudioPath) {
    return '请选择 WAV 音频文件';
  }
  if (selectedAudioPath && !/\.wav$/i.test(selectedAudioPath)) {
    return '音频格式当前只支持 WAV';
  }
  if (selectedAudioPath) {
    return null;
  }
  if (!selectedFile || !isSupportedAudioFile(selectedFile)) {
    return '音频格式当前只支持 WAV';
  }
  if (selectedFile.size <= 0) {
    return '音频文件为空';
  }
  if (selectedFile.size > MAX_AUDIO_BYTES) {
    return '音频文件不能超过 50MB';
  }
  return null;
}

function upsertJob(job: OfflineJob): void {
  const index = state.jobs.findIndex((item) => item.jobId === job.jobId);
  if (index >= 0) {
    state.jobs.splice(index, 1, job);
  } else {
    state.jobs.unshift(job);
  }
  state.currentJob = job;
}

let previewFinishListenerStarted = false;
let jobUpdateListenerStarted = false;

function startPreviewFinishListener(): void {
  if (previewFinishListenerStarted) {
    return;
  }

  previewFinishListenerStarted = true;
  void listenOfflineJobPreviewFinished((event) => {
    if (state.playingJobId === event.jobId) {
      state.playingJobId = event.playingJobId;
      state.lastMessage = `离线任务 ${event.jobId} 试听已结束`;
    }
  }).catch((error) => {
    previewFinishListenerStarted = false;
    state.lastMessage = `离线任务试听事件监听失败：${messageFromError(error)}`;
  });
}

function startJobUpdateListener(): void {
  if (jobUpdateListenerStarted) {
    return;
  }

  jobUpdateListenerStarted = true;
  void listenOfflineJobUpdated((job) => {
    upsertJob(job);
    state.lastMessage = messageForJob(job);
    state.lastError = job.status === 'failed' ? job.errorSummary : null;
  }).catch((error) => {
    jobUpdateListenerStarted = false;
    state.lastMessage = `离线任务事件监听失败：${messageFromError(error)}`;
  });
}

export function useOfflineStore() {
  startPreviewFinishListener();
  startJobUpdateListener();

  const selectedVoice = computed(() =>
    state.voices.find((voice) => voice.voiceName === state.selectedVoiceName)
  );
  const selectedEmotion = computed(() =>
    state.emotionOptions.find((emotion) => emotion.label === state.selectedEmotionLabel)
  );
  const selectedAudioLabel = computed(
    () => state.selectedAudioFileName ?? state.selectedFile?.name ?? '拖入或点击选择一段人声音频'
  );

  const canSubmit = computed(() => !state.busy && validateBeforeSubmit() === null);

  const completedJobs = computed(() => state.jobs.filter((job) => job.status === 'completed'));

  function setInputType(inputType: OfflineInputType): void {
    state.inputType = inputType;
    state.lastError = null;
    state.lastMessage = inputType === 'audio' ? '已切换到音频文件转换' : '已切换到文本生成';
  }

  function setSelectedFile(file: File | null): void {
    state.selectedFile = file;
    state.selectedAudioPath = null;
    state.selectedAudioFileName = file?.name ?? null;
    state.lastError = null;
    state.lastMessage = file ? `已选择音频：${file.name}` : '已清空音频输入';
  }

  async function chooseAudioFile(): Promise<void> {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: 'WAV Audio', extensions: ['wav'] }],
      });
      if (typeof selected !== 'string') {
        return;
      }
      state.selectedFile = null;
      state.selectedAudioPath = selected;
      state.selectedAudioFileName = fileNameFromPath(selected);
      state.lastError = null;
      state.lastMessage = `已选择音频：${state.selectedAudioFileName}`;
    } catch (error) {
      state.lastError = messageFromError(error);
      state.lastMessage = '系统文件选择器打开失败，可使用页面文件选择作为备用';
    }
  }

  function canPreviewJob(job: OfflineJob | null): boolean {
    return job?.status === 'completed' && !!job.localArtifactPath && job.outputFormat === 'wav';
  }

  function canDownloadJob(job: OfflineJob | null): boolean {
    return !!job?.localArtifactPath && job.status === 'completed';
  }

  async function load(): Promise<void> {
    state.loading = true;
    state.lastError = null;
    try {
      const [voices, jobs, emotionOptions] = await Promise.all([
        listVoices().catch(() => demoVoices),
        listOfflineJobs().catch(() => []),
        listOfflineTtsEmotions().catch(() => ({
          supportsEmotionControl: false,
          emotions: [],
        })),
      ]);
      state.voices = voices.length > 0 ? voices : demoVoices;
      state.jobs = jobs;
      state.emotionOptions = emotionOptions.supportsEmotionControl ? emotionOptions.emotions : [];
      state.currentJob = jobs[0] ?? null;
      if (
        state.selectedEmotionLabel &&
        !state.emotionOptions.some((emotion) => emotion.label === state.selectedEmotionLabel)
      ) {
        state.selectedEmotionLabel = null;
      }
      state.selectedVoiceName =
        state.selectedVoiceName &&
        state.voices.some((voice) => voice.voiceName === state.selectedVoiceName)
          ? state.selectedVoiceName
          : (state.voices[0]?.voiceName ?? null);
      state.lastMessage = `已加载 ${state.voices.length} 个音色、${state.emotionOptions.length} 个情感指令和 ${state.jobs.length} 条离线记录`;
    } catch (error) {
      state.lastError = messageFromError(error);
      state.lastMessage = '离线工作台加载失败';
    } finally {
      state.loading = false;
    }
  }

  async function submit(): Promise<void> {
    const validationError = validateBeforeSubmit();
    if (validationError) {
      state.lastError = validationError;
      return;
    }

    state.busy = true;
    state.lastError = null;
    try {
      let created: OfflineJob;
      if (state.inputType === 'audio' && state.selectedAudioPath) {
        state.lastMessage = '正在创建离线任务...';
        created = await createOfflineAudioJob({
          fileName: state.selectedAudioFileName ?? fileNameFromPath(state.selectedAudioPath),
          inputRef: state.selectedAudioPath,
          voiceName: state.selectedVoiceName!,
          runtimeParams: runtimeParams(),
          postProcessConfig: { ...state.postProcessConfig, channels: 'stereo', trimSilence: false },
          outputFormat: state.outputFormat,
        });
      } else if (state.inputType === 'audio' && state.selectedFile) {
        state.lastMessage = '正在导入音频并创建离线任务...';
        const bytes = Array.from(new Uint8Array(await state.selectedFile.arrayBuffer()));
        created = await createOfflineAudioJob({
          fileName: state.selectedFile.name,
          inputBytes: bytes,
          voiceName: state.selectedVoiceName!,
          runtimeParams: runtimeParams(),
          postProcessConfig: { ...state.postProcessConfig, channels: 'stereo', trimSilence: false },
          outputFormat: state.outputFormat,
        });
      } else {
        state.lastMessage = '正在创建文本转语音任务...';
        created = await createOfflineTextJob({
          text: state.text.trim(),
          voiceName: state.selectedVoiceName!,
          runtimeParams: runtimeParams(),
          postProcessConfig: { ...state.postProcessConfig, channels: 'stereo', trimSilence: false },
          outputFormat: state.outputFormat,
        });
      }

      upsertJob(created);
      state.lastMessage = '任务已创建，正在后台处理...';
      const started = await startOfflineJob(created.jobId);
      upsertJob(started);
      state.lastMessage = messageForJob(started);
      if (started.status === 'failed') {
        state.lastError = started.errorSummary;
      }
    } catch (error) {
      state.lastError = messageFromError(error);
      state.lastMessage = '离线任务提交失败';
    } finally {
      state.busy = false;
    }
  }

  async function retry(job: OfflineJob): Promise<void> {
    state.busy = true;
    state.lastError = null;
    try {
      const retried = await retryOfflineJob(job.jobId);
      upsertJob(retried);
      const started = await startOfflineJob(retried.jobId);
      upsertJob(started);
      state.lastMessage = messageForJob(started);
      if (started.status === 'failed') {
        state.lastError = started.errorSummary;
      }
    } catch (error) {
      state.lastError = messageFromError(error);
    } finally {
      state.busy = false;
    }
  }

  async function clearHistory(): Promise<void> {
    state.busy = true;
    state.lastError = null;
    try {
      if (state.playingJobId) {
        await stopOfflineJobPreview();
      }
      const result = await clearOfflineJobs();
      state.jobs = [];
      state.currentJob = null;
      state.playingJobId = null;
      state.lastMessage = `已清理 ${result.removedCount} 条离线记录和本地音频文件`;
    } catch (error) {
      state.lastError = messageFromError(error);
      state.lastMessage = '清理离线记录失败';
    } finally {
      state.busy = false;
    }
  }

  async function deleteJob(job: OfflineJob): Promise<void> {
    state.busy = true;
    state.lastError = null;
    try {
      if (state.playingJobId === job.jobId) {
        await stopOfflineJobPreview();
      }
      const result = await deleteOfflineJob(job.jobId);
      state.jobs = state.jobs.filter((item) => item.jobId !== result.removed.jobId);
      if (state.currentJob?.jobId === result.removed.jobId) {
        state.currentJob = state.jobs[0] ?? null;
      }
      if (state.playingJobId === result.removed.jobId) {
        state.playingJobId = null;
      }
      state.lastMessage = `已删除离线记录和音频：${result.removed.jobId}`;
    } catch (error) {
      state.lastError = messageFromError(error);
      state.lastMessage = '删除离线记录失败';
    } finally {
      state.busy = false;
    }
  }

  async function download(job: OfflineJob): Promise<void> {
    if (!canDownloadJob(job)) {
      state.lastError = '只有已完成且存在本地音频文件的离线任务可以下载';
      return;
    }

    const targetPath = await save({
      defaultPath: suggestedDownloadName(job),
      filters: [{ name: 'WAV audio', extensions: ['wav'] }],
    });
    if (!targetPath) {
      state.lastMessage = '已取消下载';
      return;
    }

    state.busy = true;
    state.lastError = null;
    try {
      const result = await downloadOfflineJob(job.jobId, targetPath);
      state.lastMessage = `已保存到：${result.targetPath}`;
    } catch (error) {
      state.lastError = messageFromError(error);
      state.lastMessage = '下载离线音频失败';
    } finally {
      state.busy = false;
    }
  }

  async function cancel(job: OfflineJob): Promise<void> {
    state.busy = true;
    try {
      const cancelled = await cancelOfflineJob(job.jobId);
      upsertJob(cancelled);
      state.lastMessage = '离线任务已取消';
    } catch (error) {
      state.lastError = messageFromError(error);
    } finally {
      state.busy = false;
    }
  }

  async function togglePreview(job: OfflineJob): Promise<void> {
    if (!canPreviewJob(job)) {
      state.lastError = '只有已完成且生成 WAV 文件的离线任务可以试听';
      return;
    }

    state.busy = true;
    state.lastError = null;
    try {
      if (state.playingJobId === job.jobId) {
        const stopped = await stopOfflineJobPreview();
        state.playingJobId = stopped.playingJobId;
        state.lastMessage = '离线任务试听已停止';
      } else {
        const playback = await toggleOfflineJobPreview(job.jobId);
        state.playingJobId = playback.playingJobId;
        state.lastMessage = playback.playingJobId
          ? `正在试听离线任务：${job.jobId}`
          : '离线任务试听已停止';
      }
    } catch (error) {
      state.lastError = messageFromError(error);
      state.lastMessage = '离线任务试听失败';
    } finally {
      state.busy = false;
    }
  }

  function selectJob(job: OfflineJob): void {
    state.currentJob = job;
    state.lastMessage = messageForJob(job);
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

  return {
    state,
    selectedVoice,
    selectedEmotion,
    selectedAudioLabel,
    canSubmit,
    completedJobs,
    load,
    setInputType,
    setSelectedFile,
    chooseAudioFile,
    submit,
    retry,
    cancel,
    selectJob,
    canPreviewJob,
    canDownloadJob,
    togglePreview,
    clearHistory,
    deleteJob,
    download,
    setPostProcessDenoise,
    setPostProcessLufs,
  };
}

function messageForJob(job: OfflineJob): string {
  if (job.status === 'completed') {
    return '离线任务已完成，可试听或下载';
  }
  if (job.status === 'failed') {
    return job.errorSummary ?? '离线任务失败';
  }
  if (job.status === 'running') {
    return `任务处理中：${job.stage}`;
  }
  if (job.status === 'cancelled') {
    return '任务已取消';
  }
  return '任务等待开始';
}

function suggestedDownloadName(job: OfflineJob): string {
  const baseName = (job.inputFileName ?? job.jobId)
    .replace(/\.[^.]+$/, '')
    .replace(/[\\/:*?"<>|]+/g, '-')
    .trim();
  return `${baseName || job.jobId}-${job.jobId}.wav`;
}
