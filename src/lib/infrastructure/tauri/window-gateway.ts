import { getCurrentWindow } from '@tauri-apps/api/window';
import type { WindowGateway } from '../../core/ports';

/** Needs capabilities core:window:allow-close / allow-minimize / allow-start-dragging (S4). */
export class TauriWindowGateway implements WindowGateway {
  close(): Promise<void> {
    return getCurrentWindow().close();
  }

  minimize(): Promise<void> {
    return getCurrentWindow().minimize();
  }

  startDragging(): Promise<void> {
    return getCurrentWindow().startDragging();
  }
}
