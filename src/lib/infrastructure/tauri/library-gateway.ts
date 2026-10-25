import type { InstallEvent, InstalledApp, UninstallOutcome } from '../../core/domain/types';
import type { LibraryGateway } from '../../core/ports';
import { call, channel } from './invoke';

// TODO(S4-bindings): swap raw invoke for generated `commands.*` and DTO types.
export class TauriLibraryGateway implements LibraryGateway {
  listInstalled(): Promise<InstalledApp[]> {
    return call<InstalledApp[]>('list_installed');
  }

  uninstall(appId: string, onEvent: (e: InstallEvent) => void): Promise<UninstallOutcome> {
    return call<UninstallOutcome>('uninstall', { appId, onEvent: channel<InstallEvent>(onEvent) });
  }

  launchApp(appId: string): Promise<void> {
    return call<void>('launch_app', { appId });
  }
}
