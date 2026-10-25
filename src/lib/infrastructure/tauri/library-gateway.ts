import type { InstallEvent, InstalledApp, UninstallOutcome } from '../../core/domain/types';
import type { LibraryGateway } from '../../core/ports';
import { commands } from './bindings';
import { channel, guard, guardVoid } from './invoke';

export class TauriLibraryGateway implements LibraryGateway {
  listInstalled(): Promise<InstalledApp[]> {
    return guard(commands.listInstalled());
  }

  uninstall(appId: string, onEvent: (e: InstallEvent) => void): Promise<UninstallOutcome> {
    return guard(commands.uninstall(appId, channel<InstallEvent>(onEvent)));
  }

  launchApp(appId: string): Promise<void> {
    return guardVoid(commands.launchApp(appId));
  }
}
