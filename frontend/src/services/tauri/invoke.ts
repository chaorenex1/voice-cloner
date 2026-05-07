import { invoke } from '@tauri-apps/api/core';

export async function invokeWithMockFallback<T>(
  command: string,
  fallback: () => T | Promise<T>,
  args?: Record<string, unknown>
): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (_error) {
    return await fallback();
  }
}
