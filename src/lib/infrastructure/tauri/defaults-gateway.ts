import type { DefaultStatus } from '../../core/domain/types';
import type { DefaultsGateway } from '../../core/ports';
import { call } from './invoke';

// TODO(S4-bindings): swap raw invoke for generated `commands.*` and DTO types.
export class TauriDefaultsGateway implements DefaultsGateway {
  status(): Promise<DefaultStatus> {
    return call<DefaultStatus>('default_handler_status');
  }

  makeDefault(): Promise<void> {
    return call<void>('make_default_handler');
  }

  dismissPrompt(): Promise<void> {
    return call<void>('dismiss_default_prompt');
  }
}
