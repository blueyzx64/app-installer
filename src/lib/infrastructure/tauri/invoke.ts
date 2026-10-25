// Thin wrapper over @tauri-apps/api so every gateway rejects with a CommandError.
// TODO(S4-bindings): when bindings.ts carries all commands, call `commands.*` instead of raw invoke.
import { Channel, invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { toCommandError } from '../../core/domain/types';

export async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    throw toCommandError(e);
  }
}

/** A Tauri Channel that forwards each message to `onMessage`. */
export function channel<T>(onMessage: (m: T) => void): Channel<T> {
  const ch = new Channel<T>();
  ch.onmessage = onMessage;
  return ch;
}

/** Synchronous unsubscribe for an async `listen`. Safe to call before the listener is ready. */
export function subscribe<P>(event: string, cb: (payload: P) => void): () => void {
  let stopped = false;
  let unlisten: (() => void) | null = null;
  void listen<P>(event, (e) => cb(e.payload)).then((fn) => {
    if (stopped) fn();
    else unlisten = fn;
  });
  return () => {
    stopped = true;
    unlisten?.();
    unlisten = null;
  };
}
