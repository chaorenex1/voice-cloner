import type {
  CreateOfflineAudioJobRequest,
  CreateOfflineTextJobRequest,
  OfflineJob,
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
