import { invoke } from '@tauri-apps/api/core';
import type { InstallEvent, InstalledApp, UninstallOutcome } from '../../core/domain/types';
import type { LibraryGateway, UninstallPlan } from '../../core/ports';
import { commands } from './bindings';
import { channel, guard, guardVoid } from './invoke';

export class TauriLibraryGateway implements LibraryGateway {
  listInstalled(): Promise<InstalledApp[]> {
    return guard(commands.listInstalled());
  }

  // TODO(v2:S4): use commands.uninstallPlan once bindings are regenerated.
  uninstallPlan(appId: string): Promise<UninstallPlan> {
    return guard(invoke<UninstallPlan>('uninstall_plan', { appId }));
  }

  uninstall(appId: string, onEvent: (e: InstallEvent) => void): Promise<UninstallOutcome> {
    return guard(commands.uninstall(appId, channel<InstallEvent>(onEvent)));
  }

  launchApp(appId: string): Promise<void> {
    return guardVoid(commands.launchApp(appId));
  }
}
