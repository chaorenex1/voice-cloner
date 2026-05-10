import type { RealtimeSession, RealtimeStreamSnapshot } from './types/realtime';

const DEBUG_STORAGE_KEY = 'voice-cloner:debug:realtime';

function debugOverrideEnabled(): boolean {
  if (typeof window === 'undefined') {
    return false;
  }

  try {
    const value = window.localStorage.getItem(DEBUG_STORAGE_KEY);
    return value === '1' || value === 'true' || value === 'verbose';
  } catch {
    return false;
  }
}

export function isRealtimeDebugEnabled(): boolean {
  return import.meta.env.DEV || debugOverrideEnabled();
}

export function logRealtimeDebug(message: string, data?: unknown): void {
  if (!isRealtimeDebugEnabled()) {
    return;
  }

  const prefix = `[RealtimeVoice][${new Date().toISOString()}] ${message}`;
  if (data === undefined) {
    console.debug(prefix);
    return;
  }

  console.debug(prefix, data);
}

export function logRealtimeError(message: string, error: unknown, data?: unknown): void {
  const prefix = `[RealtimeVoice][${new Date().toISOString()}] ${message}`;
  console.error(prefix, {
    error: error instanceof Error ? error.message : String(error),
    data,
  });
}

export function summarizeRealtimeSession(session: RealtimeSession): Record<string, unknown> {
  return {
    sessionId: session.sessionId,
    traceId: session.traceId,
    voiceName: session.voiceName,
    status: session.status,
    websocketUrl: session.websocketUrl,
    runtimeParams: session.runtimeParams,
    errorSummary: session.errorSummary,
    updatedAt: session.updatedAt,
  };
}

export function summarizeRealtimeSnapshot(
  snapshot: RealtimeStreamSnapshot
): Record<string, unknown> {
  return {
    sessionId: snapshot.sessionId,
    websocketState: snapshot.websocketState,
    taskId: snapshot.taskId,
    audioMode: snapshot.audioMode,
    configuredVoiceName: snapshot.configuredVoiceName,
    sentFrames: snapshot.sentFrames,
    receivedFrames: snapshot.receivedFrames,
    sentBytes: snapshot.sentBytes,
    receivedBytes: snapshot.receivedBytes,
    latencyMs: snapshot.latencyMs,
    inputLevel: snapshot.inputLevel,
    inputState: snapshot.inputState,
    inputSource: snapshot.inputSource,
    inputHealth: snapshot.inputHealth,
    monitorState: snapshot.monitorState,
    virtualMicFrames: snapshot.virtualMicFrames,
    monitorFrames: snapshot.monitorFrames,
    outputReceivedFrames: snapshot.outputReceivedFrames,
    outputWrittenFrames: snapshot.outputWrittenFrames,
    outputAckMismatches: snapshot.outputAckMismatches,
    outputPlaybackQueueMs: snapshot.outputPlaybackQueueMs,
    outputLastFrameGapMs: snapshot.outputLastFrameGapMs,
    outputMaxFrameGapMs: snapshot.outputMaxFrameGapMs,
    outputGapSkips: snapshot.outputGapSkips,
    outputLateDrops: snapshot.outputLateDrops,
    outputOverflowDrops: snapshot.outputOverflowDrops,
    outputDuplicateDrops: snapshot.outputDuplicateDrops,
    outputPlayableFrames: snapshot.outputPlayableFrames,
    firstOutputLatencyMs: snapshot.firstOutputLatencyMs,
    rustSentSeq: snapshot.rustSentSeq,
    serverDequeuedSeq: snapshot.serverDequeuedSeq,
    asrCommittedSegments: snapshot.asrCommittedSegments,
    asrCommittedAudioMs: snapshot.asrCommittedAudioMs,
    asrSegmentId: snapshot.asrSegmentId,
    asrFirstFrameSeq: snapshot.asrFirstFrameSeq,
    asrLastFrameSeq: snapshot.asrLastFrameSeq,
    asrCommitReason: snapshot.asrCommitReason,
    asrQueueMs: snapshot.asrQueueMs,
    ledgerEntries: snapshot.ledger.length,
    vadSpeechFrames: snapshot.vadSpeechFrames,
    vadUtterancesEnded: snapshot.vadUtterancesEnded,
    ttsAudioChunks: snapshot.ttsAudioChunks,
    asrCommittedChars: snapshot.asrCommittedChars,
    ttsQueuedJobs: snapshot.ttsQueuedJobs,
    ttsStartedJobs: snapshot.ttsStartedJobs,
    ttsCompletedJobs: snapshot.ttsCompletedJobs,
    ttsDroppedJobs: snapshot.ttsDroppedJobs,
    ttsQueuedChars: snapshot.ttsQueuedChars,
    ttsStartedChars: snapshot.ttsStartedChars,
    ttsCompletedChars: snapshot.ttsCompletedChars,
    ttsDroppedChars: snapshot.ttsDroppedChars,
    serverRealtimeConfig: snapshot.serverRealtimeConfig,
    convertedFrames: snapshot.convertedFrames,
    pipelineStage: snapshot.pipelineStage,
    asrText: snapshot.asrText,
    ttsTextChunks: snapshot.ttsTextChunks,
    lastEvent: snapshot.lastEvent,
    protocolEvent: snapshot.protocolEvent,
    lastPrompt: snapshot.lastPrompt,
    eventSeq: snapshot.eventSeq,
    ttsJobId: snapshot.ttsJobId,
    audioChunkIndex: snapshot.audioChunkIndex,
    backpressureHint: snapshot.backpressureHint,
    lastError: snapshot.lastError,
  };
}
