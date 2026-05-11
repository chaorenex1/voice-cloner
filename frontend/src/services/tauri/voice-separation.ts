import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type {
  CreateVoiceSeparationJobRequest,
  CustomVoiceProfileResult,
  ReferenceAudioTranscription,
  SaveSeparatedVocalsRequest,
  StartVoiceSeparationJobRequest,
  VoiceSeparationDownloadResult,
  VoiceSeparationJob,
  VoiceSeparationPreviewState,
  VoiceSeparationRuntimeStatus,
  VoiceSeparationStemName,
} from '../../utils/types/voice-separation';

export async function checkVoiceSeparationRuntime(): Promise<VoiceSeparationRuntimeStatus> {
  return invoke<VoiceSeparationRuntimeStatus>('check_voice_separation_runtime');
}

export async function createVoiceSeparationJob(
  request: CreateVoiceSeparationJobRequest
): Promise<VoiceSeparationJob> {
  return invoke<VoiceSeparationJob>('create_voice_separation_job', { request });
}

export async function startVoiceSeparationJob(
  jobId: string,
  request?: StartVoiceSeparationJobRequest
): Promise<VoiceSeparationJob> {
  return invoke<VoiceSeparationJob>('start_voice_separation_job', { jobId, request });
}

export async function getVoiceSeparationJob(jobId: string): Promise<VoiceSeparationJob> {
  return invoke<VoiceSeparationJob>('get_voice_separation_job', { jobId });
}

export async function listVoiceSeparationJobs(): Promise<VoiceSeparationJob[]> {
  return invoke<VoiceSeparationJob[]>('list_voice_separation_jobs');
}

export async function cancelVoiceSeparationJob(jobId: string): Promise<VoiceSeparationJob> {
  return invoke<VoiceSeparationJob>('cancel_voice_separation_job', { jobId });
}

export async function deleteVoiceSeparationJob(jobId: string): Promise<{ jobId: string; message: string }> {
  return invoke<{ jobId: string; message: string }>('delete_voice_separation_job', { jobId });
}

export async function previewVoiceSeparationStem(
  jobId: string,
  stem: VoiceSeparationStemName
): Promise<VoiceSeparationPreviewState> {
  return invoke<VoiceSeparationPreviewState>('preview_voice_separation_stem', {
    jobId,
    request: { stem },
  });
}

export async function stopVoiceSeparationPreview(): Promise<VoiceSeparationPreviewState> {
  return invoke<VoiceSeparationPreviewState>('stop_voice_separation_preview');
}

export async function downloadVoiceSeparationStem(
  jobId: string,
  stem: VoiceSeparationStemName,
  targetPath: string
): Promise<VoiceSeparationDownloadResult> {
  return invoke<VoiceSeparationDownloadResult>('download_voice_separation_stem', {
    jobId,
    request: { stem, targetPath },
  });
}

export async function transcribeSeparatedVocals(jobId: string): Promise<ReferenceAudioTranscription> {
  return invoke<ReferenceAudioTranscription>('transcribe_separated_vocals', { jobId });
}

export async function saveSeparatedVocalsAsCustomVoice(
  jobId: string,
  request: SaveSeparatedVocalsRequest
): Promise<CustomVoiceProfileResult> {
  return invoke<CustomVoiceProfileResult>('save_separated_vocals_as_custom_voice', {
    jobId,
    request,
  });
}

export function listenVoiceSeparationJobUpdated(
  handler: (job: VoiceSeparationJob) => void
): Promise<() => void> {
  return listen<VoiceSeparationJob>('voice-separation-job-updated', (event) => handler(event.payload));
}

export function listenVoiceSeparationPreviewFinished(
  handler: (state: VoiceSeparationPreviewState) => void
): Promise<() => void> {
  return listen<VoiceSeparationPreviewState>('voice-separation-preview-finished', (event) => handler(event.payload));
}
