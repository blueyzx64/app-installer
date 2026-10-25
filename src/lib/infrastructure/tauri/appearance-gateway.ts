import type { Appearance } from '../../core/domain/types';
import type { AppearanceGateway } from '../../core/ports';
import { commands } from './bindings';
import { guard, subscribe } from './invoke';

export class TauriAppearanceGateway implements AppearanceGateway {
  get(): Promise<Appearance> {
    return guard(commands.getAppearance());
  }

  onChange(cb: (a: Appearance) => void): () => void {
    return subscribe<Appearance>('appearance://changed', cb);
  }
}
