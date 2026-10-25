import type { InstallEvent, InstallOutcome, PackagePreview } from '../../core/domain/types';
import type { InstallerGateway } from '../../core/ports';
import { call, channel, subscribe } from './invoke';

// TODO(S4-bindings): swap raw invoke for generated `commands.*` and DTO types.
export class TauriInstallerGateway implements InstallerGateway {
  takeLaunchFile(): Promise<string | null> {
    return call<string | null>('take_launch_file');
  }

  openPackage(path: string): Promise<PackagePreview> {
    return call<PackagePreview>('open_package', { path });
  }

  install(previewId: string, onEvent: (e: InstallEvent) => void): Promise<InstallOutcome> {
    return call<InstallOutcome>('install', { previewId, onEvent: channel<InstallEvent>(onEvent) });
  }

  cancelInstall(previewId: string): Promise<void> {
    return call<void>('cancel_install', { previewId });
  }

  onOpenFile(cb: (path: string) => void): () => void {
    return subscribe<{ path: string }>('app://open-file', (p) => cb(p.path));
  }
}
