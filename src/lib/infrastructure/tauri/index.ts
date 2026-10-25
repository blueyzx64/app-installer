import type { Gateways } from '../../core/ports';
import { TauriAppearanceGateway } from './appearance-gateway';
import { TauriDefaultsGateway } from './defaults-gateway';
import { TauriFilesGateway } from './files-gateway'; // lane S4b
import { TauriInstallerGateway } from './installer-gateway';
import { TauriLibraryGateway } from './library-gateway';
import { TauriWindowGateway } from './window-gateway';

export function createTauriGateways(): Gateways {
  return {
    installer: new TauriInstallerGateway(),
    library: new TauriLibraryGateway(),
    defaults: new TauriDefaultsGateway(),
    appearance: new TauriAppearanceGateway(),
    window: new TauriWindowGateway(),
    files: new TauriFilesGateway() // lane S4b
  };
}
