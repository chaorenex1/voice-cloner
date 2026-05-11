import { invoke } from '@tauri-apps/api/core';
import type {
  CreateRealtimeSessionRequest,
  RealtimeFullChainTestReport,
  RealtimeFullChainTestRequest,
  RealtimeSession,
  RealtimeStreamSnapshot,
  StartRealtimeFileInputRequest,
  RuntimeParams,
} from '../../utils/types/realtime';
import {
  logRealtimeDebug,
  logRealtimeError,
  summarizeRealtimeSession,
  summarizeRealtimeSnapshot,
} from '../../utils/realtime-debug';

function isTauriRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

async function invokeRealtime<T>(
  command: string,
  args: Record<string, unknown>,
  fallback: () => T
): Promise<T> {
  const startedAt = performance.now();
  const tauriRuntime = isTauriRuntime();
  const runtime = tauriRuntime ? 'tauri' : 'mock';
  logRealtimeDebug(`ipc:${command}:start`, { runtime, args: summarizeRealtimeArgs(args) });

  if (!tauriRuntime) {
    const response = fallback();
    logRealtimeDebug(`ipc:${command}:mock-success`, {
      durationMs: Math.round(performance.now() - startedAt),
      response: summarizeRealtimeResponse(response),
    });
    return response;
  }

  try {
    const response = await invoke<T>(command, args);
    logRealtimeDebug(`ipc:${command}:success`, {
      durationMs: Math.round(performance.now() - startedAt),
      response: summarizeRealtimeResponse(response),
    });
    return response;
  } catch (error) {
    logRealtimeError(`ipc:${command}:error`, error, {
      durationMs: Math.round(performance.now() - startedAt),
      args: summarizeRealtimeArgs(args),
    });
    throw error;
  }
}

function summarizeRealtimeResponse(response: unknown): unknown {
  if (isRealtimeSession(response)) {
    return summarizeRealtimeSession(response);
  }
  if (isRealtimeSnapshot(response)) {
    return summarizeRealtimeSnapshot(response);
  }
  if (isRealtimeFullChainReport(response)) {
    return {
      sessionId: response.sessionId,
      verdict: response.summary.verdict,
      reasons: response.summary.reasons,
      samples: response.timeline.length,
      outputReceivedFrames: response.summary.outputReceivedFrames,
      outputWrittenFrames: response.summary.outputWrittenFrames,
      outputMaxFrameGapMs: response.summary.outputMaxFrameGapMs,
      playbackOutputMode: response.playbackOutputMode,
    };
  }
  return response;
}

function summarizeRealtimeArgs(args: Record<string, unknown>): Record<string, unknown> {
  return JSON.parse(
    JSON.stringify(args, (key, value) => {
      if (key === 'audioBytes' && Array.isArray(value)) {
        return { byteCount: value.length };
      }
      return value;
    })
  ) as Record<string, unknown>;
}

function isRealtimeSession(value: unknown): value is RealtimeSession {
  return (
    typeof value === 'object' &&
    value !== null &&
    'sessionId' in value &&
    'traceId' in value &&
    'status' in value
  );
}

function isRealtimeSnapshot(value: unknown): value is RealtimeStreamSnapshot {
  return (
    typeof value === 'object' &&
    value !== null &&
    'sessionId' in value &&
    'websocketState' in value &&
    'sentFrames' in value
  );
}

function isRealtimeFullChainReport(value: unknown): value is RealtimeFullChainTestReport {
  return (
    typeof value === 'object' &&
    value !== null &&
    'summary' in value &&
    'timeline' in value &&
    'playbackOutputMode' in value
  );
}

function mockSession(request: CreateRealtimeSessionRequest): RealtimeSession {
  const now = new Date().toISOString();
  return {
    sessionId: `preview-session-${Date.now()}`,
    traceId: 'preview-realtime',
    voiceName: request.voiceName,
    runtimeParams: request.runtimeParams,
    postProcessConfig: request.postProcessConfig ?? null,
    status: 'idle',
    websocketUrl: 'ws://127.0.0.1:8000/ws/v1/realtime/voice',
    errorSummary: null,
    createdAt: now,
    updatedAt: now,
  };
}

function mockSnapshot(session: RealtimeSession): RealtimeStreamSnapshot {
  return {
    sessionId: session.sessionId,
    websocketUrl: session.websocketUrl,
    websocketState: session.status === 'running' ? 'running' : 'preview',
    taskId: 'preview-task',
    audioMode: 'passthrough',
    configuredVoiceName: session.voiceName,
    sentFrames: session.status === 'running' ? 42 : 0,
    receivedFrames: session.status === 'running' ? 42 : 0,
    sentBytes: session.status === 'running' ? 80640 : 0,
    receivedBytes: session.status === 'running' ? 80640 : 0,
    latencyMs: session.status === 'running' ? 24 : null,
    inputLevel: { rms: 0, peak: 0 },
    inputState: 'off',
    inputSource: 'microphone',
    inputHealth: null,
    monitorState: 'off',
    virtualMicFrames: session.status === 'running' ? 42 : 0,
    monitorFrames: 0,
    outputReceivedFrames: session.status === 'running' ? 42 : 0,
    outputWrittenFrames: 0,
    outputAckMismatches: 0,
    outputPlaybackQueueMs: 0,
    outputLastFrameGapMs: session.status === 'running' ? 20 : null,
    outputMaxFrameGapMs: session.status === 'running' ? 28 : null,
    outputGapSkips: 0,
    outputLateDrops: 0,
    outputOverflowDrops: 0,
    outputDuplicateDrops: 0,
    outputPlayableFrames: session.status === 'running' ? 42 : 0,
    firstOutputLatencyMs: session.status === 'running' ? 1200 : null,
    lastOutputAtMs: null,
    rustSentSeq: session.status === 'running' ? 42 : null,
    serverDequeuedSeq: session.status === 'running' ? 42 : null,
    asrCommittedSegments: session.status === 'running' ? 2 : 0,
    asrCommittedAudioMs: session.status === 'running' ? 1600 : 0,
    asrSegmentId: session.status === 'running' ? 'utt_1_seg_2' : null,
    asrFirstFrameSeq: session.status === 'running' ? 41 : null,
    asrLastFrameSeq: session.status === 'running' ? 80 : null,
    asrCommitReason: session.status === 'running' ? 'partial' : null,
    asrQueueMs: session.status === 'running' ? 0 : null,
    ledger: session.status === 'running'
      ? [
          {
            timestampMs: Date.now(),
            stage: 'asr_segment',
            event: 'asr.segment_committed',
            status: 'partial',
            message: 'duration=800ms frames=40',
            inputFrameSeq: null,
            rustSentSeq: null,
            serverDequeuedSeq: null,
            asrSegmentId: 'utt_1_seg_2',
            asrFirstFrameSeq: 41,
            asrLastFrameSeq: 80,
            asrCommitReason: 'partial',
            asrQueueMs: 0,
            ttsRevisionId: null,
            ttsJobId: null,
            audioChunkIndex: null,
            playbackQueueMs: null,
          },
        ]
      : [],
    vadSpeechFrames: session.status === 'running' ? 21 : 0,
    vadUtterancesEnded: 0,
    ttsAudioChunks: session.status === 'running' ? 42 : 0,
    convertedFrames: session.status === 'running' ? 42 : 0,
    pipelineStage: session.status === 'running' ? 'preview_audio_received' : 'preview',
    asrText: null,
    ttsTextChunks: 0,
    lastEvent: session.status === 'running' ? 'configured' : null,
    protocolEvent: session.status === 'running' ? 'session.configured' : null,
    lastPrompt: session.status === 'running' ? '音色已就绪，可以打开麦克风' : null,
    eventSeq: null,
    serverTsMs: null,
    schemaVersion: session.status === 'running' ? 'realtime_voice.v1' : null,
    utteranceId: null,
    hypothesisId: null,
    revisionId: null,
    ttsJobId: null,
    audioChunkIndex: null,
    configVersion: null,
    serverRealtimeConfig: null,
    asrCommittedText: null,
    asrCommittedChars: 0,
    ttsQueuedJobs: 0,
    ttsStartedJobs: 0,
    ttsCompletedJobs: 0,
    ttsDroppedJobs: 0,
    ttsQueuedChars: 0,
    ttsStartedChars: 0,
    ttsCompletedChars: 0,
    ttsDroppedChars: 0,
    backpressureHint: null,
    lastError: null,
  };
}

export async function createRealtimeSession(
  request: CreateRealtimeSessionRequest
): Promise<RealtimeSession> {
  return invokeRealtime('create_realtime_session', { request }, () => mockSession(request));
}

export async function startRealtimeSession(session: RealtimeSession): Promise<RealtimeSession> {
  return invokeRealtime('start_realtime_session', { sessionId: session.sessionId }, () => ({
    ...session,
    status: 'running' as const,
    updatedAt: new Date().toISOString(),
  }));
}

export async function stopRealtimeSession(session: RealtimeSession): Promise<RealtimeSession> {
  return invokeRealtime('stop_realtime_session', { sessionId: session.sessionId }, () => ({
    ...session,
    status: 'stopped' as const,
    updatedAt: new Date().toISOString(),
  }));
}

export async function startRealtimeInput(
  session: RealtimeSession
): Promise<RealtimeStreamSnapshot> {
  return invokeRealtime('start_realtime_input', { sessionId: session.sessionId }, () => ({
    ...mockSnapshot(session),
    inputState: 'capturing',
  }));
}

export async function startRealtimeFileInput(
  session: RealtimeSession,
  request: StartRealtimeFileInputRequest
): Promise<RealtimeStreamSnapshot> {
  return invokeRealtime(
    'start_realtime_file_input',
    { sessionId: session.sessionId, request },
    () => ({
      ...mockSnapshot(session),
      inputState: 'capturing',
      inputSource: 'local_file',
      inputHealth: `正在模拟播放本地音频: ${request.fileName}`,
    })
  );
}

export async function stopRealtimeInput(session: RealtimeSession): Promise<RealtimeStreamSnapshot> {
  return invokeRealtime('stop_realtime_input', { sessionId: session.sessionId }, () => ({
    ...mockSnapshot(session),
    inputState: 'off',
  }));
}

export async function startRealtimeMonitor(
  session: RealtimeSession
): Promise<RealtimeStreamSnapshot> {
  return invokeRealtime('start_realtime_monitor', { sessionId: session.sessionId }, () => ({
    ...mockSnapshot(session),
    monitorState: 'listening',
    monitorFrames: 12,
  }));
}

export async function stopRealtimeMonitor(
  session: RealtimeSession
): Promise<RealtimeStreamSnapshot> {
  return invokeRealtime('stop_realtime_monitor', { sessionId: session.sessionId }, () => ({
    ...mockSnapshot(session),
    monitorState: 'off',
  }));
}

export async function updateRealtimeParams(
  sessionId: string,
  runtimeParams: RuntimeParams
): Promise<RealtimeSession> {
  return invokeRealtime(
    'update_realtime_params',
    { sessionId, request: { runtimeParams } },
    () =>
      ({
        sessionId,
        traceId: 'preview-realtime',
        voiceName: 'preview',
        runtimeParams,
        postProcessConfig: null,
        status: 'running',
        websocketUrl: 'ws://127.0.0.1:8000/ws/v1/realtime/voice',
        errorSummary: null,
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      }) satisfies RealtimeSession
  );
}

export async function switchRealtimeVoice(
  sessionId: string,
  voiceName: string
): Promise<RealtimeSession> {
  return invokeRealtime(
    'switch_realtime_voice',
    { sessionId, request: { voiceName } },
    () =>
      ({
        sessionId,
        traceId: 'preview-realtime',
        voiceName,
        runtimeParams: { values: {} },
        postProcessConfig: null,
        status: 'running',
        websocketUrl: 'ws://127.0.0.1:8000/ws/v1/realtime/voice',
        errorSummary: null,
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      }) satisfies RealtimeSession
  );
}

export async function getRealtimeStreamSnapshot(
  session: RealtimeSession
): Promise<RealtimeStreamSnapshot> {
  return invokeRealtime('get_realtime_stream_snapshot', { sessionId: session.sessionId }, () =>
    mockSnapshot(session)
  );
}

export async function runRealtimeFullChainSimulation(
  request: RealtimeFullChainTestRequest
): Promise<RealtimeFullChainTestReport> {
  return invokeRealtime('run_realtime_full_chain_simulation', { request }, () => {
    const session = mockSession({
      voiceName: request.voiceName,
      runtimeParams: request.runtimeParams ?? { values: {} },
      postProcessConfig: undefined,
    });
    const running = { ...session, status: 'running' as const };
    const snapshot = {
      ...mockSnapshot(running),
      inputSource: 'local_file',
      outputWrittenFrames: 42,
      monitorFrames: request.startMonitor === false ? 0 : 42,
    };
    return {
      sessionId: session.sessionId,
      traceId: session.traceId,
      voiceName: request.voiceName,
      websocketUrl: session.websocketUrl,
      fileName: request.fileName,
      audioBytes: request.audioBytes.length,
      sampleRate: 48000,
      frameMs: 20,
      playbackOutputMode: request.startMonitor === false ? 'buffer_only' : 'monitor',
      monitorStartError: null,
      timeline: [{ elapsedMs: 1000, snapshot }],
      summary: {
        verdict: request.startMonitor === false ? 'degraded' : 'pass',
        reasons: request.startMonitor === false ? ['浏览器预览未打开监听输出'] : [],
        durationMs: 1000,
        sentFrames: snapshot.sentFrames,
        receivedFrames: snapshot.receivedFrames,
        outputReceivedFrames: snapshot.outputReceivedFrames,
        outputPlayableFrames: snapshot.outputPlayableFrames,
        outputWrittenFrames: snapshot.outputWrittenFrames,
        monitorFrames: snapshot.monitorFrames,
        virtualMicFrames: snapshot.virtualMicFrames,
        outputAckMismatches: snapshot.outputAckMismatches,
        outputGapSkips: snapshot.outputGapSkips,
        outputLateDrops: snapshot.outputLateDrops,
        outputOverflowDrops: snapshot.outputOverflowDrops,
        outputDuplicateDrops: snapshot.outputDuplicateDrops,
        firstOutputLatencyMs: snapshot.firstOutputLatencyMs,
        outputMaxFrameGapMs: snapshot.outputMaxFrameGapMs,
        maxPlaybackQueueMs: snapshot.outputPlaybackQueueMs,
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
        lastEvent: snapshot.lastEvent,
        lastError: snapshot.lastError,
      },
    };
  });
}
