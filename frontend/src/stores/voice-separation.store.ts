import { computed, reactive } from 'vue';
import { open, save } from '@tauri-apps/plugin-dialog';
import { invalidateCustomVoiceCache } from '../services/tauri/voice-library';
import {
  cancelVoiceSeparationJob,
  checkVoiceSeparationRuntime,
  createVoiceSeparationJob,
  deleteVoiceSeparationJob,
  downloadVoiceSeparationStem,
  getVoiceSeparationJob,
  listenVoiceSeparationPreviewFinished,
  listenVoiceSeparationJobUpdated,
  listVoiceSeparationJobs,
  previewVoiceSeparationStem,
  saveSeparatedVocalsAsCustomVoice,
  startVoiceSeparationJob,
  stopVoiceSeparationPreview,
  transcribeSeparatedVocals,
} from '../services/tauri/voice-separation';
import type {
  CustomVoiceProfileResult,
  VoicePostProcessConfig,
  VoiceSeparationJob,
  VoiceSeparationModel,
  VoiceSeparationRuntimeStatus,
  VoiceSeparationStemName,
} from '../utils/types/voice-separation';
import { defaultPostProcessConfig } from '../utils/types/voice-separation';
import { useVoiceLibraryStore } from './voice-library.store';

export interface SaveVoiceDialogState {
  open: boolean;
  jobId: string | null;
  voiceName: string;
  busy: boolean;
}

export interface VoiceSeparationState {
  runtime: VoiceSeparationRuntimeStatus | null;
  sourcePath: string | null;
  sourceFileName: string | null;
  model: VoiceSeparationModel;
  postProcessConfig: VoicePostProcessConfig;
  jobs: VoiceSeparationJob[];
  currentJob: VoiceSeparationJob | null;
  expandedJobIds: string[];
  visibleJobCount: number;
  playingJobId: string | null;
  playingStem: VoiceSeparationStemName | null;
  saveDialog: SaveVoiceDialogState;
  loading: boolean;
  busy: boolean;
  lastMessage: string;
  lastError: string | null;
}

const state = reactive<VoiceSeparationState>({
  runtime: null,
  sourcePath: null,
  sourceFileName: null,
  model: 'htDemucs',
  postProcessConfig: { ...defaultPostProcessConfig },
  jobs: [],
  currentJob: null,
  expandedJobIds: [],
  visibleJobCount: 8,
  playingJobId: null,
  playingStem: null,
  saveDialog: {
    open: false,
    jobId: null,
    voiceName: '',
    busy: false,
  },
  loading: false,
  busy: false,
  lastMessage: '人声分离工作台等待加载',
  lastError: null,
});

let updateListenerStarted = false;
let previewFinishListenerStarted = false;
let pollTimer: number | null = null;

function messageFromError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function fileNameFromPath(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}

function upsertJob(job: VoiceSeparationJob): void {
  const index = state.jobs.findIndex((item) => item.jobId === job.jobId);
  if (index >= 0) {
    state.jobs.splice(index, 1, job);
  } else {
    state.jobs.unshift(job);
  }
  state.currentJob = job;
}

function isTerminal(job: VoiceSeparationJob | null): boolean {
  return !!job && ['ready', 'saved', 'failed', 'cancelled'].includes(job.status);
}

function startUpdateListener(): void {
  if (updateListenerStarted) {
    return;
  }
  updateListenerStarted = true;
  void listenVoiceSeparationJobUpdated((job) => {
    upsertJob(job);
    state.lastMessage = messageForJob(job);
    if (job.status === 'failed') {
      state.lastError = job.errorMessage ?? '人声分离失败';
    }
    stopPollingIfDone(job);
  }).catch((error) => {
    updateListenerStarted = false;
    state.lastError = messageFromError(error);
  });
}

function startPreviewFinishListener(): void {
  if (previewFinishListenerStarted) {
    return;
  }
  previewFinishListenerStarted = true;
  void listenVoiceSeparationPreviewFinished((playback) => {
    state.playingJobId = playback.playingJobId ?? null;
    state.playingStem = playback.playingStem ?? null;
    state.lastMessage = '试听已结束';
  }).catch((error) => {
    previewFinishListenerStarted = false;
    state.lastError = messageFromError(error);
  });
}

function startPolling(jobId: string): void {
  stopPolling();
  pollTimer = window.setInterval(() => {
    void getVoiceSeparationJob(jobId)
      .then((job) => {
        upsertJob(job);
        state.lastMessage = messageForJob(job);
        stopPollingIfDone(job);
      })
      .catch(() => undefined);
  }, 1500);
}

function stopPollingIfDone(job: VoiceSeparationJob): void {
  if (isTerminal(job)) {
    stopPolling();
    state.busy = false;
  }
}

function stopPolling(): void {
  if (pollTimer !== null) {
    window.clearInterval(pollTimer);
    pollTimer = null;
  }
}

function runningJob(): VoiceSeparationJob | null {
  return state.jobs.find((job) => !isTerminal(job)) ?? null;
}

function isJobPlaying(jobId: string): boolean {
  return state.playingJobId === jobId;
}

async function refreshVoiceLibraryAfterSave(voiceName: string): Promise<void> {
  invalidateCustomVoiceCache();
  const voiceLibrary = useVoiceLibraryStore();
  await voiceLibrary.loadVoices({ preserveOperation: true });
  await voiceLibrary.selectVoice(voiceName);
}

export function useVoiceSeparationStore() {
  startUpdateListener();
  startPreviewFinishListener();

  const canStart = computed(() => !!state.sourcePath && !state.busy);
  const visibleJobs = computed(() => state.jobs.slice(0, state.visibleJobCount));
  const hasMoreJobs = computed(() => state.visibleJobCount < state.jobs.length);
  const startButtonText = computed(() => {
    const job = runningJob();
    if (!job) {
      return '开始人声分离';
    }
    return `${job.currentStageMessage} ${Math.round(job.progress * 100)}%`;
  });

  async function load(): Promise<void> {
    state.loading = true;
    state.lastError = null;
    try {
      const [runtime, jobs] = await Promise.all([
        checkVoiceSeparationRuntime(),
        listVoiceSeparationJobs(),
      ]);
      state.runtime = runtime;
      state.jobs = jobs;
      state.currentJob = jobs[0] ?? null;
      state.lastMessage = runtime.warnings.length
        ? (runtime.warnings[0] ?? '人声分离运行时存在警告')
        : `已加载 ${jobs.length} 条人声分离任务`;
    } catch (error) {
      state.lastError = messageFromError(error);
      state.lastMessage = '加载人声分离运行时失败';
    } finally {
      state.loading = false;
    }
  }

  async function chooseSource(): Promise<void> {
    state.lastError = null;
    const selected = await open({
      multiple: false,
      filters: [
        {
          name: 'Video or Audio',
          extensions: [
            'mp4',
            'mov',
            'mkv',
            'webm',
            'avi',
            'wav',
            'mp3',
            'm4a',
            'aac',
            'flac',
            'ogg',
            'aiff',
            'aif',
          ],
        },
      ],
    });
    if (typeof selected !== 'string') {
      state.lastMessage = '已取消选择源材料';
      return;
    }
    state.sourcePath = selected;
    state.sourceFileName = fileNameFromPath(selected);
    state.lastMessage = `已选择源材料：${state.sourceFileName}`;
  }

  function setModel(model: VoiceSeparationModel): void {
    state.model = model;
  }

  function updatePostProcessConfig(patch: Partial<VoicePostProcessConfig>): void {
    state.postProcessConfig = { ...state.postProcessConfig, ...patch, trimSilence: false };
  }

  async function start(): Promise<void> {
    if (!state.sourcePath) {
      state.lastError = '请先选择源材料';
      return;
    }
    state.busy = true;
    state.lastError = null;
    try {
      const postProcessConfig = { ...state.postProcessConfig, trimSilence: false };
      const created = await createVoiceSeparationJob({
        sourcePath: state.sourcePath,
        model: state.model,
        postProcessConfig,
      });
      upsertJob(created);
      state.expandedJobIds = state.expandedJobIds.filter((id) => id !== created.jobId);
      state.lastMessage = '任务已创建，正在后台分离人声...';
      await startVoiceSeparationJob(created.jobId, { postProcessConfig });
      startPolling(created.jobId);
    } catch (error) {
      state.lastError = messageFromError(error);
      state.lastMessage = '人声分离任务启动失败';
      state.busy = false;
    }
  }

  async function refreshJob(job: VoiceSeparationJob): Promise<void> {
    const fresh = await getVoiceSeparationJob(job.jobId);
    upsertJob(fresh);
  }

  function toggleJobExpanded(jobId: string): void {
    state.expandedJobIds = state.expandedJobIds.includes(jobId)
      ? state.expandedJobIds.filter((id) => id !== jobId)
      : [...state.expandedJobIds, jobId];
  }

  function isJobExpanded(jobId: string): boolean {
    return state.expandedJobIds.includes(jobId);
  }

  function loadMoreResults(): void {
    state.visibleJobCount += 8;
  }

  async function togglePreview(
    job: VoiceSeparationJob,
    stem: VoiceSeparationStemName
  ): Promise<void> {
    state.busy = true;
    state.lastError = null;
    try {
      if (state.playingJobId === job.jobId && state.playingStem === stem) {
        const stopped = await stopVoiceSeparationPreview();
        state.playingJobId = stopped.playingJobId ?? null;
        state.playingStem = stopped.playingStem ?? null;
        state.lastMessage = '试听已停止';
      } else {
        const playback = await previewVoiceSeparationStem(job.jobId, stem);
        state.playingJobId = playback.playingJobId ?? null;
        state.playingStem = playback.playingStem ?? null;
        state.lastMessage = playback.playingJobId ? `正在试听 ${stem}` : '试听已停止';
      }
    } catch (error) {
      state.lastError = messageFromError(error);
      state.lastMessage = '试听分离结果失败';
    } finally {
      state.busy = false;
    }
  }

  async function downloadStem(
    job: VoiceSeparationJob,
    stem: VoiceSeparationStemName
  ): Promise<void> {
    const targetPath = await save({
      defaultPath: suggestedStemFileName(job, stem),
      filters: [{ name: 'WAV audio', extensions: ['wav'] }],
    });
    if (!targetPath) {
      state.lastMessage = '已取消下载';
      return;
    }
    state.busy = true;
    state.lastError = null;
    try {
      const result = await downloadVoiceSeparationStem(job.jobId, stem, targetPath);
      state.lastMessage = `已保存到：${result.targetPath}`;
    } catch (error) {
      state.lastError = messageFromError(error);
      state.lastMessage = '下载分离音频失败';
    } finally {
      state.busy = false;
    }
  }

  function openSaveVoiceDialog(job: VoiceSeparationJob): void {
    state.saveDialog.open = true;
    state.saveDialog.jobId = job.jobId;
    state.saveDialog.voiceName = job.voiceName ?? job.sourceFileName.replace(/\.[^.]+$/, '');
    state.saveDialog.busy = false;
  }

  function closeSaveVoiceDialog(): void {
    state.saveDialog.open = false;
    state.saveDialog.jobId = null;
    state.saveDialog.voiceName = '';
    state.saveDialog.busy = false;
  }

  async function confirmSaveVoice(): Promise<CustomVoiceProfileResult | null> {
    const jobId = state.saveDialog.jobId;
    const voiceName = state.saveDialog.voiceName.trim();
    if (!jobId || !voiceName) {
      state.lastError = '请输入音色名称';
      return null;
    }
    state.saveDialog.busy = true;
    state.busy = true;
    state.lastError = null;
    try {
      let job =
        state.jobs.find((item) => item.jobId === jobId) ?? (await getVoiceSeparationJob(jobId));
      let referenceText = job.referenceText?.trim() ?? '';
      if (!referenceText) {
        state.lastMessage = '正在自动识别人声参考文本...';
        const result = await transcribeSeparatedVocals(jobId);
        referenceText = result.text.trim();
        job = await getVoiceSeparationJob(jobId);
        upsertJob(job);
      }
      if (!referenceText) {
        throw new Error('自动识别参考文本为空，无法保存自定义音色');
      }
      const profile = await saveSeparatedVocalsAsCustomVoice(jobId, {
        voiceName,
        referenceText,
        voiceInstruction: '',
      });
      await refreshVoiceLibraryAfterSave(profile.voiceName);
      await refreshJob(job);
      state.lastMessage =
        profile.syncStatus === 'synced'
          ? `已保存并同步 FunSpeech：${profile.voiceName}`
          : `已保存自定义音色：${profile.voiceName}，FunSpeech 同步状态：${profile.syncStatus}`;
      closeSaveVoiceDialog();
      return profile;
    } catch (error) {
      state.lastError = messageFromError(error);
      state.lastMessage = '保存自定义音色失败';
      return null;
    } finally {
      state.saveDialog.busy = false;
      state.busy = false;
    }
  }

  async function cancelCurrent(): Promise<void> {
    const job = runningJob() ?? state.currentJob;
    if (!job) {
      return;
    }
    state.busy = true;
    try {
      const cancelled = await cancelVoiceSeparationJob(job.jobId);
      upsertJob(cancelled);
      state.lastMessage = '人声分离任务已取消';
    } catch (error) {
      state.lastError = messageFromError(error);
    } finally {
      stopPolling();
      state.busy = false;
    }
  }

  async function deleteJob(job: VoiceSeparationJob): Promise<void> {
    if (!isTerminal(job)) {
      state.lastError = '运行中的人声分离任务不能删除';
      return;
    }
    state.busy = true;
    state.lastError = null;
    try {
      if (isJobPlaying(job.jobId)) {
        const stopped = await stopVoiceSeparationPreview();
        state.playingJobId = stopped.playingJobId ?? null;
        state.playingStem = stopped.playingStem ?? null;
      }
      const result = await deleteVoiceSeparationJob(job.jobId);
      state.jobs = state.jobs.filter((item) => item.jobId !== result.jobId);
      state.expandedJobIds = state.expandedJobIds.filter((id) => id !== result.jobId);
      if (state.currentJob?.jobId === result.jobId) {
        state.currentJob = state.jobs[0] ?? null;
      }
      state.lastMessage = result.message;
    } catch (error) {
      state.lastError = messageFromError(error);
      state.lastMessage = '删除人声分离任务失败';
    } finally {
      state.busy = false;
    }
  }

  function selectJob(job: VoiceSeparationJob): void {
    state.currentJob = job;
    state.lastMessage = messageForJob(job);
  }

  return {
    state,
    canStart,
    visibleJobs,
    hasMoreJobs,
    startButtonText,
    load,
    chooseSource,
    setModel,
    updatePostProcessConfig,
    start,
    refreshJob,
    toggleJobExpanded,
    isJobExpanded,
    loadMoreResults,
    togglePreview,
    downloadStem,
    openSaveVoiceDialog,
    closeSaveVoiceDialog,
    confirmSaveVoice,
    cancelCurrent,
    deleteJob,
    selectJob,
  };
}

function messageForJob(job: VoiceSeparationJob): string {
  if (job.status === 'ready') {
    return '人声分离完成，可展开试听或保存人声为自定义音色';
  }
  if (job.status === 'saved') {
    return '已保存为自定义音色';
  }
  if (job.status === 'failed') {
    return job.errorMessage ?? '人声分离失败';
  }
  if (job.status === 'cancelled') {
    return '人声分离任务已取消';
  }
  return job.currentStageMessage;
}

function suggestedStemFileName(job: VoiceSeparationJob, stem: VoiceSeparationStemName): string {
  const baseName = job.sourceFileName
    .replace(/\.[^.]+$/, '')
    .replace(/[\\/:*?"<>|]+/g, '-')
    .trim();
  return `${baseName || job.jobId}-${stem}.wav`;
}
