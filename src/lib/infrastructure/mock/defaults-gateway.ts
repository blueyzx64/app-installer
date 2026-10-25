// lane S13: mock DefaultsGateway with ?default=yes|no|asked scenarios.
import type { DefaultStatus } from '../../core/domain/types';
import type { DefaultsGateway } from '../../core/ports';

export type DefaultScenario = 'yes' | 'no' | 'asked';

export function parseDefaultScenario(search: string): DefaultScenario | null {
  const v = new URLSearchParams(search).get('default');
  return v === 'yes' || v === 'no' || v === 'asked' ? v : null;
}

export function defaultScenarioFromLocation(): DefaultScenario | null {
  const loc = (globalThis as { location?: { search?: string } }).location;
  return loc?.search ? parseDefaultScenario(loc.search) : null;
}

/** yes = already default, asked = declined before, no (and unset) = first launch. */
export function defaultStatusFor(s: DefaultScenario | null): DefaultStatus {
  return { isDefault: s === 'yes', askedBefore: s === 'yes' || s === 'asked' };
}

export function createMockDefaults(status: DefaultStatus): DefaultsGateway {
  return {
    async status() {
      return { ...status };
    },
    async makeDefault() {
      status.isDefault = true;
      status.askedBefore = true;
    },
    async dismissPrompt() {
      status.askedBefore = true;
    }
  };
}
