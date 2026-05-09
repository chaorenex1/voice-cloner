import type {
  CreateOfflineAudioJobRequest,
  CreateOfflineTextJobRequest,
  OfflineJob,
  TtsEmotionOptions,
} from '../../utils/types/offline';
import { listen } from '@tauri-apps/api/event';
import { invokeWithMockFallback } from './invoke';

function nowIso(): string {
  return new Date().toISOString();
}

function mockJob(patch: Partial<OfflineJob>): OfflineJob {
  const now = nowIso();
  return {
    jobId: `preview-offline-${Date.now()}`,
    traceId: 'preview-offline',
    inputType: 'text',
    inputRef: '',
    inputFileName: null,
    voiceName: 'preview',
    runtimeParams: { values: {} },
    outputFormat: 'wav',
    status: 'created',
    stage: 'created',
    progress: 0,
    artifactUrl: null,
    localArtifactPath: null,
    errorSummary: null,
    createdAt: now,
    updatedAt: now,
    ...patch,
  };
}

const fallbackTtsEmotions: TtsEmotionOptions = {
  supportsEmotionControl: true,
  emotions: [
    { id: 'neutral', label: '自然平静', prompt: '请用自然平静的语气说这句话。' },
    { id: 'happy', label: '开心愉悦', prompt: '请用开心、愉悦的语气说这句话。' },
    { id: 'sad', label: '伤心低落', prompt: '请用伤心、低落的语气说这句话。' },
    { id: 'angry', label: '生气强烈', prompt: '请用生气、强烈的语气说这句话。' },
    { id: 'fearful', label: '害怕紧张', prompt: '请用害怕、紧张的语气说这句话。' },
    { id: 'disgusted', label: '厌恶不满', prompt: '请用厌恶、不满的语气说这句话。' },
    { id: 'surprised', label: '惊讶意外', prompt: '请用惊讶、意外的语气说这句话。' },
  ],
};

export async function listOfflineTtsEmotions(): Promise<TtsEmotionOptions> {
  return invokeWithMockFallback('list_offline_tts_emotions', () => fallbackTtsEmotions);
}

export async function createOfflineAudioJob(
  request: CreateOfflineAudioJobRequest
): Promise<OfflineJob> {
  return invokeWithMockFallback(
    'create_offline_audio_job',
    () =>
      mockJob({
        inputType: 'audio',
        inputRef: request.fileName,
        inputFileName: request.fileName,
        voiceName: request.voiceName,
        runtimeParams: request.runtimeParams,
        outputFormat: request.outputFormat ?? 'wav',
        artifactUrl: 'http://127.0.0.1:8000/stream/v1/asr',
      }),
    { request }
  );
}

export async function createOfflineTextJob(
  request: CreateOfflineTextJobRequest
): Promise<OfflineJob> {
  return invokeWithMockFallback(
    'create_offline_text_job',
    () =>
      mockJob({
        inputType: 'text',
        inputRef: request.text,
        voiceName: request.voiceName,
        runtimeParams: request.runtimeParams,
        outputFormat: request.outputFormat ?? 'wav',
        artifactUrl: 'http://127.0.0.1:8000/stream/v1/tts',
      }),
    { request }
  );
}

export async function startOfflineJob(jobId: string): Promise<OfflineJob> {
  return invokeWithMockFallback(
    'start_offline_job',
    () =>
      mockJob({
        jobId,
        status: 'running',
        stage: 'preparing',
        progress: 5,
      }),
    { jobId }
  );
}

export async function cancelOfflineJob(jobId: string): Promise<OfflineJob> {
  return invokeWithMockFallback(
    'cancel_offline_job',
    () => mockJob({ jobId, status: 'cancelled', stage: 'cancelled' }),
    { jobId }
  );
}

export async function retryOfflineJob(jobId: string): Promise<OfflineJob> {
  return invokeWithMockFallback(
    'retry_offline_job',
    () => mockJob({ jobId, status: 'created', stage: 'created', progress: 0 }),
    { jobId }
  );
}

export async function getOfflineJob(jobId: string): Promise<OfflineJob> {
  return invokeWithMockFallback('get_offline_job', () => mockJob({ jobId }), { jobId });
}

export async function listOfflineJobs(): Promise<OfflineJob[]> {
  return invokeWithMockFallback('list_offline_jobs', () => []);
}

export interface OfflineJobsClearResult {
  removedCount: number;
}

export async function clearOfflineJobs(): Promise<OfflineJobsClearResult> {
  return invokeWithMockFallback('clear_offline_jobs', () => ({ removedCount: 0 }));
}

export interface OfflineJobDeleteResult {
  removed: OfflineJob;
}

export async function deleteOfflineJob(jobId: string): Promise<OfflineJobDeleteResult> {
  return invokeWithMockFallback('delete_offline_job', () => ({ removed: mockJob({ jobId }) }), {
    jobId,
  });
}

export interface OfflineJobDownloadResult {
  targetPath: string;
}

export async function downloadOfflineJob(
  jobId: string,
  targetPath: string
): Promise<OfflineJobDownloadResult> {
  return invokeWithMockFallback('download_offline_job', () => ({ targetPath }), {
    jobId,
    request: {
      targetPath,
    },
  });
}

export interface OfflineJobPreviewState {
  playingJobId: string | null;
}

export interface OfflineJobPreviewFinishedEvent {
  jobId: string;
  playingJobId: string | null;
}

export async function toggleOfflineJobPreview(jobId: string): Promise<OfflineJobPreviewState> {
  return invokeWithMockFallback('toggle_offline_job_preview', () => ({ playingJobId: jobId }), {
    request: {
      jobId,
    },
  });
}

export async function stopOfflineJobPreview(): Promise<OfflineJobPreviewState> {
  return invokeWithMockFallback('stop_offline_job_preview', () => ({ playingJobId: null }));
}

export function listenOfflineJobPreviewFinished(
  handler: (event: OfflineJobPreviewFinishedEvent) => void
): Promise<() => void> {
  return listen<OfflineJobPreviewFinishedEvent>('offline-job-preview-finished', (event) => {
    handler(event.payload);
  });
}

export function listenOfflineJobUpdated(handler: (job: OfflineJob) => void): Promise<() => void> {
  return listen<OfflineJob>('offline-job-updated', (event) => {
    handler(event.payload);
  });
}
