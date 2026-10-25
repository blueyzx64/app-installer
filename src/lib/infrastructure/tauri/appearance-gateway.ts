import type { Appearance } from '../../core/domain/types';
import type { AppearanceGateway } from '../../core/ports';
import { call, subscribe } from './invoke';

// TODO(S4-bindings): swap raw invoke for generated `commands.*` and DTO types.
export class TauriAppearanceGateway implements AppearanceGateway {
  get(): Promise<Appearance> {
    return call<Appearance>('get_appearance');
  }

  onChange(cb: (a: Appearance) => void): () => void {
    return subscribe<Appearance>('appearance://changed', cb);
  }
}
