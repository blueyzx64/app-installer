// Mock LibraryGateway. Scenarios: empty-library (seed), fail-offline / cancel-permission (removal
// outcome, see install-script), and `?extras=N` to change how many other items a system removal takes.
import type { CommandError, InstalledApp, UninstallOutcome } from '../../core/domain/types';
import type { LibraryGateway, UninstallPlan } from '../../core/ports';
import { uninstallScript } from './install-script';
import type { Scenario } from './scenario';

/** Other items a system removal takes along when `?extras=` is not set. */
export const DEFAULT_EXTRAS = 2;

export function extrasFromLocation(): number | null {
  const loc = (globalThis as { location?: { search?: string } }).location;
  const v = new URLSearchParams(loc?.search ?? '').get('extras');
  const n = v === null ? NaN : Number(v);
  return Number.isInteger(n) && n >= 0 ? n : null;
}

export interface MockLibraryWorld {
  scenario: Scenario;
  getInstalled(): InstalledApp[];
  setInstalled(apps: InstalledApp[]): void;
  launched: string[];
  /** Runs an uninstall script; resolves with its outcome. */
  run(
    steps: ReturnType<typeof uninstallScript>,
    onEvent: Parameters<LibraryGateway['uninstall']>[1]
  ): Promise<UninstallOutcome>;
  extras?: number | null;
}

function fail(code: CommandError['code'], detail: string | null = null): CommandError {
  return { code, detail };
}

export function createMockLibrary(w: MockLibraryWorld): LibraryGateway {
  const find = (id: string) => {
    const app = w.getInstalled().find((a) => a.id === id);
    if (!app) throw fail('notFound', 'mock: unknown app');
    return app;
  };
  return {
    async listInstalled() {
      return [...w.getInstalled()];
    },
    async uninstallPlan(appId): Promise<UninstallPlan> {
      const system = find(appId).source === 'systemPackage';
      const extras = w.extras ?? extrasFromLocation() ?? DEFAULT_EXTRAS;
      return { alsoRemoves: system ? extras : 0, needsPermission: system };
    },
    async uninstall(appId, onEvent) {
      const app = find(appId);
      const outcome = await w.run(
        uninstallScript(app.source === 'systemPackage', w.scenario),
        onEvent
      );
      if (outcome.kind === 'removed') {
        w.setInstalled(w.getInstalled().filter((a) => a.id !== appId));
      }
      return outcome;
    },
    async launchApp(appId) {
      find(appId);
      w.launched.push(appId);
    }
  };
}
