import type { DefaultStatus } from '../../core/domain/types';
import type { DefaultsGateway } from '../../core/ports';
import { commands } from './bindings';
import { guard, guardVoid } from './invoke';

export class TauriDefaultsGateway implements DefaultsGateway {
  status(): Promise<DefaultStatus> {
    return guard(commands.defaultHandlerStatus());
  }

  makeDefault(): Promise<void> {
    return guardVoid(commands.makeDefaultHandler());
  }

  dismissPrompt(): Promise<void> {
    return guardVoid(commands.dismissDefaultPrompt());
  }
}
