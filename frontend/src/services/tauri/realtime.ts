import { invoke } from '@tauri-apps/api/core';
import type {
  CreateRealtimeSessionRequest,
  RealtimeSession,
  RealtimeStreamSnapshot,
  RuntimeParams,
} from '../../utils/types/realtime';

function isTauriRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

async function invokeRealtime<T>(
  command: string,
  args: Record<string, unknown>,
  fallback: () => T
): Promise<T> {
  if (!isTauriRuntime()) {
    return fallback();
  }
  return invoke<T>(command, args);
}

function mockSession(request: CreateRealtimeSessionRequest): RealtimeSession {
  const now = new Date().toISOString();
  return {
    sessionId: `preview-session-${Date.now()}`,
    traceId: 'preview-realtime',
    voiceName: request.voiceName,
    runtimeParams: request.runtimeParams,
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
    virtualMicFrames: session.status === 'running' ? 42 : 0,
    lastEvent: session.status === 'running' ? 'configured' : null,
    lastError: null,
  };
}

export async function createRealtimeSession(
  request: CreateRealtimeSessionRequest
): Promise<RealtimeSession> {
  return invokeRealtime('create_realtime_session', { request }, () => mockSession(request));
}

export async function startRealtimeSession(session: RealtimeSession): Promise<RealtimeSession> {
  return invokeRealtime(
    'start_realtime_session',
    { sessionId: session.sessionId },
    () => ({ ...session, status: 'running' as const, updatedAt: new Date().toISOString() }),
  );
}

export async function stopRealtimeSession(session: RealtimeSession): Promise<RealtimeSession> {
  return invokeRealtime(
    'stop_realtime_session',
    { sessionId: session.sessionId },
    () => ({ ...session, status: 'stopped' as const, updatedAt: new Date().toISOString() }),
  );
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
        status: 'running',
        websocketUrl: 'ws://127.0.0.1:8000/ws/v1/realtime/voice',
        errorSummary: null,
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      }) satisfies RealtimeSession,
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
        status: 'running',
        websocketUrl: 'ws://127.0.0.1:8000/ws/v1/realtime/voice',
        errorSummary: null,
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      }) satisfies RealtimeSession,
  );
}

export async function getRealtimeStreamSnapshot(
  session: RealtimeSession
): Promise<RealtimeStreamSnapshot> {
  return invokeRealtime(
    'get_realtime_stream_snapshot',
    { sessionId: session.sessionId },
    () => mockSnapshot(session),
  );
}
