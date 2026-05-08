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

export function summarizeRealtimeSnapshot(snapshot: RealtimeStreamSnapshot): Record<string, unknown> {
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
    virtualMicFrames: snapshot.virtualMicFrames,
    lastEvent: snapshot.lastEvent,
    lastError: snapshot.lastError,
  };
}
