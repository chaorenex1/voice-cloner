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

export interface RealtimeLedgerEntry {
  timestampMs: number;
  stage: string;
  event: string;
  status: string | null;
  message: string | null;
  inputFrameSeq: number | null;
  rustSentSeq: number | null;
  serverDequeuedSeq: number | null;
  asrSegmentId: string | null;
  asrFirstFrameSeq: number | null;
  asrLastFrameSeq: number | null;
  asrCommitReason: string | null;
  asrQueueMs: number | null;
  ttsRevisionId: number | null;
  ttsJobId: string | null;
  audioChunkIndex: number | null;
  playbackQueueMs: number | null;
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
  inputState: string;
  inputSource: 'microphone' | 'local_file' | string;
  inputHealth: string | null;
  monitorState: string;
  virtualMicFrames: number;
  monitorFrames: number;
  outputReceivedFrames: number;
  outputWrittenFrames: number;
  outputAckMismatches: number;
  outputPlaybackQueueMs: number;
  outputLastFrameGapMs: number | null;
  outputMaxFrameGapMs: number | null;
  outputGapSkips: number;
  outputLateDrops: number;
  outputOverflowDrops: number;
  outputDuplicateDrops: number;
  outputPlayableFrames: number;
  firstOutputLatencyMs: number | null;
  lastOutputAtMs: number | null;
  rustSentSeq: number | null;
  serverDequeuedSeq: number | null;
  asrCommittedSegments: number;
  asrCommittedAudioMs: number;
  asrSegmentId: string | null;
  asrFirstFrameSeq: number | null;
  asrLastFrameSeq: number | null;
  asrCommitReason: string | null;
  asrQueueMs: number | null;
  ledger: RealtimeLedgerEntry[];
  vadSpeechFrames: number;
  vadUtterancesEnded: number;
  ttsAudioChunks: number;
  convertedFrames: number;
  pipelineStage: string;
  asrText: string | null;
  ttsTextChunks: number;
  lastEvent: string | null;
  protocolEvent: string | null;
  lastPrompt: string | null;
  eventSeq: number | null;
  serverTsMs: number | null;
  schemaVersion: string | null;
  utteranceId: string | null;
  hypothesisId: string | null;
  revisionId: number | null;
  ttsJobId: string | null;
  audioChunkIndex: number | null;
  configVersion: number | null;
  serverRealtimeConfig: Record<string, unknown> | null;
  asrCommittedText: string | null;
  asrCommittedChars: number;
  ttsQueuedJobs: number;
  ttsStartedJobs: number;
  ttsCompletedJobs: number;
  ttsDroppedJobs: number;
  ttsQueuedChars: number;
  ttsStartedChars: number;
  ttsCompletedChars: number;
  ttsDroppedChars: number;
  backpressureHint: string | null;
  lastError: string | null;
}

export interface StartRealtimeFileInputRequest {
  fileName: string;
  audioBytes: number[];
}

export interface RealtimeFullChainTestRequest {
  voiceName: string;
  fileName: string;
  audioBytes: number[];
  runtimeParams?: RuntimeParams;
  backendBaseUrl?: string | null;
  startMonitor?: boolean | null;
  pollIntervalMs?: number | null;
  drainGraceMs?: number | null;
  maxDurationMs?: number | null;
}

export type RealtimeFullChainVerdict = 'pass' | 'degraded' | 'fail';

export interface RealtimeFullChainTimelineSample {
  elapsedMs: number;
  snapshot: RealtimeStreamSnapshot;
}

export interface RealtimeFullChainSummary {
  verdict: RealtimeFullChainVerdict;
  reasons: string[];
  durationMs: number;
  sentFrames: number;
  receivedFrames: number;
  outputReceivedFrames: number;
  outputPlayableFrames: number;
  outputWrittenFrames: number;
  monitorFrames: number;
  virtualMicFrames: number;
  outputAckMismatches: number;
  outputGapSkips: number;
  outputLateDrops: number;
  outputOverflowDrops: number;
  outputDuplicateDrops: number;
  firstOutputLatencyMs: number | null;
  outputMaxFrameGapMs: number | null;
  maxPlaybackQueueMs: number;
  vadSpeechFrames: number;
  vadUtterancesEnded: number;
  ttsAudioChunks: number;
  asrCommittedChars: number;
  ttsQueuedJobs: number;
  ttsStartedJobs: number;
  ttsCompletedJobs: number;
  ttsDroppedJobs: number;
  ttsQueuedChars: number;
  ttsStartedChars: number;
  ttsCompletedChars: number;
  ttsDroppedChars: number;
  lastEvent: string | null;
  lastError: string | null;
}

export interface RealtimeFullChainTestReport {
  sessionId: string;
  traceId: string;
  voiceName: string;
  websocketUrl: string;
  fileName: string;
  audioBytes: number;
  sampleRate: number;
  frameMs: number;
  playbackOutputMode: string;
  monitorStartError: string | null;
  timeline: RealtimeFullChainTimelineSample[];
  summary: RealtimeFullChainSummary;
}
