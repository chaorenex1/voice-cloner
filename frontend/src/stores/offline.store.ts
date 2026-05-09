import { computed, reactive } from 'vue';
import {
  cancelOfflineJob,
  createOfflineAudioJob,
  createOfflineTextJob,
  listenOfflineJobPreviewFinished,
  listOfflineJobs,
  retryOfflineJob,
  startOfflineJob,
  stopOfflineJobPreview,
  toggleOfflineJobPreview,
} from '../services/tauri/offline';
import { listVoices } from '../services/tauri/voice-library';
import type { OfflineInputType, OfflineJob, RuntimeParams } from '../utils/types/offline';
import type { VoiceSummary } from '../utils/types/voice';

export interface OfflineParamState {
  pitchRate: number;
  speechRate: number;
  volume: number;
}

export interface OfflineState {
  inputType: OfflineInputType;
  text: string;
  selectedFile: File | null;
  selectedVoiceName: string | null;
  outputFormat: 'wav';
  params: OfflineParamState;
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
    voiceName: 'preview',
    displayName: '预览音色',
    source: 'remote',
    tags: ['预览'],
    hasReferenceAudio: false,
    updatedAt: 'preview',
    referenceTextPreview: '浏览器预览模式',
    syncStatus: 'synced',
  },
];

const state = reactive<OfflineState>({
  inputType: 'audio',
  text: '',
  selectedFile: null,
  selectedVoiceName: null,
  outputFormat: 'wav',
  params: {
    pitchRate: 0,
    speechRate: 0,
    volume: 50,
  },
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
  return {
    values: {
      pitchRate: state.params.pitchRate,
      speechRate: state.params.speechRate,
      volume: state.params.volume,
    },
  };
}

function messageFromError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function isSupportedAudioFile(file: File): boolean {
  return /\.wav$/i.test(file.name);
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

  if (!state.selectedFile) {
    return '请选择 WAV 音频文件';
  }
  if (!isSupportedAudioFile(state.selectedFile)) {
    return '音频格式当前只支持 WAV';
  }
  if (state.selectedFile.size <= 0) {
    return '音频文件为空';
  }
  if (state.selectedFile.size > MAX_AUDIO_BYTES) {
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

export function useOfflineStore() {
  startPreviewFinishListener();

  const selectedVoice = computed(() =>
    state.voices.find((voice) => voice.voiceName === state.selectedVoiceName)
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
    state.lastError = null;
    state.lastMessage = file ? `已选择音频：${file.name}` : '已清空音频输入';
  }

  function canPreviewJob(job: OfflineJob | null): boolean {
    return job?.status === 'completed' && !!job.localArtifactPath && job.outputFormat === 'wav';
  }

  async function load(): Promise<void> {
    state.loading = true;
    state.lastError = null;
    try {
      const [voices, jobs] = await Promise.all([
        listVoices().catch(() => demoVoices),
        listOfflineJobs().catch(() => []),
      ]);
      state.voices = voices.length > 0 ? voices : demoVoices;
      state.jobs = jobs;
      state.currentJob = jobs[0] ?? null;
      state.selectedVoiceName =
        state.selectedVoiceName &&
        state.voices.some((voice) => voice.voiceName === state.selectedVoiceName)
          ? state.selectedVoiceName
          : (state.voices[0]?.voiceName ?? null);
      state.lastMessage = `已加载 ${state.voices.length} 个音色和 ${state.jobs.length} 条离线记录`;
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
      if (state.inputType === 'audio' && state.selectedFile) {
        state.lastMessage = '正在导入音频并创建离线任务...';
        const bytes = Array.from(new Uint8Array(await state.selectedFile.arrayBuffer()));
        created = await createOfflineAudioJob({
          fileName: state.selectedFile.name,
          inputBytes: bytes,
          voiceName: state.selectedVoiceName!,
          runtimeParams: runtimeParams(),
          outputFormat: state.outputFormat,
        });
      } else {
        state.lastMessage = '正在创建文本转语音任务...';
        created = await createOfflineTextJob({
          text: state.text.trim(),
          voiceName: state.selectedVoiceName!,
          runtimeParams: runtimeParams(),
          outputFormat: state.outputFormat,
        });
      }

      upsertJob(created);
      state.lastMessage = '任务已创建，正在处理...';
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

  return {
    state,
    selectedVoice,
    canSubmit,
    completedJobs,
    load,
    setInputType,
    setSelectedFile,
    submit,
    retry,
    cancel,
    selectJob,
    canPreviewJob,
    togglePreview,
  };
}

function messageForJob(job: OfflineJob): string {
  if (job.status === 'completed') {
    return `离线任务已完成：${job.localArtifactPath ?? '等待导出路径'}`;
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
