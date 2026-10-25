/** True when running inside the Tauri webview (the IPC bridge is injected by Tauri). */
export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}
