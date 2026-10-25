import { invoke } from '@tauri-apps/api/core';
import type { FilesGateway } from '../../core/ports/files-gateway';
import { guard } from './invoke';

export class TauriFilesGateway implements FilesGateway {
  pickPackage(): Promise<string | null> {
    // TODO(S4-bindings): switch to commands.pickPackage() once bindings.ts is regenerated.
    return guard(invoke<string | null>('pick_package'));
  }
}
