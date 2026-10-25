import type { InstallEvent, InstallOutcome, PackagePreview } from '../../core/domain/types';
import type { InstallerGateway } from '../../core/ports';
import { commands } from './bindings';
import { channel, guard, guardVoid, subscribe } from './invoke';

export class TauriInstallerGateway implements InstallerGateway {
  takeLaunchFile(): Promise<string | null> {
    return guard(commands.takeLaunchFile());
  }

  openPackage(path: string): Promise<PackagePreview> {
    return guard(commands.openPackage(path));
  }

  install(previewId: string, onEvent: (e: InstallEvent) => void): Promise<InstallOutcome> {
    return guard(commands.install(previewId, channel<InstallEvent>(onEvent)));
  }

  cancelInstall(previewId: string): Promise<void> {
    return guardVoid(commands.cancelInstall(previewId));
  }

  onOpenFile(cb: (path: string) => void): () => void {
    return subscribe<{ path: string }>('app://open-file', (p) => cb(p.path));
  }
}
