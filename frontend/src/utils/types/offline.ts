export type OfflineInputType = 'audio' | 'text';

export type OfflineJobStatus = 'created' | 'running' | 'completed' | 'failed' | 'cancelled';

export interface RuntimeParams {
  values: Record<string, unknown>;
}

export interface OfflineJob {
  jobId: string;
  traceId: string;
  inputType: OfflineInputType;
  inputRef: string;
  inputFileName: string | null;
  voiceName: string;
  runtimeParams: RuntimeParams;
  outputFormat: 'wav';
  status: OfflineJobStatus;
  stage: string;
  progress: number;
  artifactUrl: string | null;
  localArtifactPath: string | null;
  errorSummary: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface CreateOfflineAudioJobRequest {
  fileName: string;
  inputBytes: number[];
  voiceName: string;
  runtimeParams: RuntimeParams;
  outputFormat?: 'wav';
}

export interface CreateOfflineTextJobRequest {
  text: string;
  voiceName: string;
  runtimeParams: RuntimeParams;
  outputFormat?: 'wav';
}
