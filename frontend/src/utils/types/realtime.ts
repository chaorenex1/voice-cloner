export type RealtimeSessionStatus =
  | 'idle'
  | 'connecting'
  | 'running'
  | 'stopping'
  | 'stopped'
  | 'failed';

export interface RuntimeParams {
  values: Record<string, unknown>;
}

export interface RealtimeSession {
  sessionId: string;
  traceId: string;
  voiceName: string;
  runtimeParams: RuntimeParams;
  status: RealtimeSessionStatus;
  websocketUrl: string;
  errorSummary: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface CreateRealtimeSessionRequest {
  voiceName: string;
  runtimeParams: RuntimeParams;
}

export interface RealtimeStreamSnapshot {
  sessionId: string;
  websocketUrl: string;
  websocketState: string;
  taskId: string | null;
  audioMode: string | null;
  configuredVoiceName: string;
  sentFrames: number;
  receivedFrames: number;
  sentBytes: number;
  receivedBytes: number;
  latencyMs: number | null;
  inputLevel: {
    rms: number;
    peak: number;
  };
  virtualMicFrames: number;
  pipelineStage: string;
  asrText: string | null;
  ttsTextChunks: number;
  lastEvent: string | null;
  lastError: string | null;
}
