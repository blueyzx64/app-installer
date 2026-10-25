// Helpers over the generated bindings. Wrappers throw CommandError (D-013); guard() normalises
// anything else (plain strings, Errors) into one so controllers can rely on the shape.
import { Channel } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { toCommandError } from '../../core/domain/types';

export async function guard<T>(p: Promise<T>): Promise<T> {
  try {
    return await p;
  } catch (e) {
    throw toCommandError(e);
  }
}

/** guard() for commands that return nothing (specta gives `null`). */
export async function guardVoid(p: Promise<unknown>): Promise<void> {
  await guard(p);
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
