// Timer-driven fake gateways: `pnpm dev` in a plain browser runs every flow.
import type {
  Appearance,
  CommandError,
  DefaultStatus,
  InstallEvent,
  InstallOutcome,
  InstalledApp,
  PackagePreview,
  UninstallOutcome
} from '../../core/domain/types';
import type {
  AppearanceGateway,
  DefaultsGateway,
  Gateways,
  InstallerGateway,
  LibraryGateway,
  WindowGateway
} from '../../core/ports';
// lane S13
import {
  createMockDefaults,
  defaultScenarioFromLocation,
  defaultStatusFor
} from './defaults-gateway';
import {
  LUMEN_FILES,
  formatForScenario,
  formatFromPath,
  lumenApp,
  lumenPreview,
  seedLibrary
} from './fixtures';
import { installScript, uninstallScript, type ScriptStep } from './install-script';
import {
  DEFAULT_SCENARIO,
  launchOverrideFromLocation,
  scenarioFromLocation,
  type Scenario
} from './scenario';

export { SCENARIOS, parseScenario, type Scenario } from './scenario';

export interface MockOptions {
  /** Defaults to `?scenario=` from the URL, else rpm. */
  scenario?: Scenario | null;
  /** Path returned once by takeLaunchFile. `null` = no launch file. Default: per scenario. */
  launchFile?: string | null;
}

/** Test/dev handles that real gateways don't have. */
export interface MockControls {
  scenario: Scenario;
  emitOpenFile(path: string): void;
  emitAppearance(a: Appearance): void;
  launched: string[];
  windowCalls: string[];
  defaults: DefaultStatus;
}

export type MockGateways = Gateways & { controls: MockControls };

function fail(code: CommandError['code'], detail: string | null = null): CommandError {
  return { code, detail };
}

/** Run a script with setTimeout; resolve with its outcome. `cancel()` ends it early. */
function runScript<O>(
  steps: ScriptStep<O>[],
  onEvent: (e: InstallEvent) => void
): { done: Promise<O>; cancel: (o: O) => void } {
  const timers: ReturnType<typeof setTimeout>[] = [];
  let finish!: (o: O) => void;
  let settled = false;
  const done = new Promise<O>((res) => {
    finish = (o) => {
      if (settled) return;
      settled = true;
      timers.forEach(clearTimeout);
      res(o);
    };
  });
  for (const step of steps) {
    timers.push(
      setTimeout(() => {
        if (settled) return;
        if (step.event) onEvent(step.event);
        if (step.outcome !== undefined) finish(step.outcome);
      }, step.at)
    );
  }
  return { done, cancel: finish };
}

export function createMockGateways(options: MockOptions = {}): MockGateways {
  const scenario: Scenario = options.scenario ?? scenarioFromLocation() ?? DEFAULT_SCENARIO;
  const explicitScenario = options.scenario ?? scenarioFromLocation();
  const override = launchOverrideFromLocation();

  // --- shared world ---
  let installed: InstalledApp[] = seedLibrary(scenario);
  let previewSeq = 0;
  const previews = new Map<string, PackagePreview>();
  const runs = new Map<string, { cancel: (o: InstallOutcome) => void }>();
  const launched: string[] = [];
  const windowCalls: string[] = [];
  const openFileCbs = new Set<(path: string) => void>();
  const appearanceCbs = new Set<(a: Appearance) => void>();
  let launchFile: string | null =
    options.launchFile !== undefined
      ? options.launchFile
      : override.none || scenario === 'empty-library'
        ? null
        : (override.file ?? LUMEN_FILES[formatForScenario(scenario)]);
  const defaultStatus: DefaultStatus = defaultStatusFor(defaultScenarioFromLocation()); // lane S13
  let appearance: Appearance = {
    colorScheme: 'light',
    accent: null,
    highContrast: false,
    reducedMotion: false,
    controls: { side: 'right', buttons: ['close'] }
  };

  const installer: InstallerGateway = {
    async takeLaunchFile() {
      const f = launchFile;
      launchFile = null; // consumed once
      return f;
    },
    openPackage(path) {
      return new Promise<PackagePreview>((resolve, reject) => {
        setTimeout(() => {
          const byName = formatFromPath(path);
          if (!byName && !path.trim()) return reject(fail('corruptFile', 'mock: empty path'));
          // An explicit ?scenario= wins; otherwise infer the format from the file name.
          const format = explicitScenario
            ? formatForScenario(scenario)
            : (byName ?? formatForScenario(scenario));
          const effective: Scenario = explicitScenario
            ? scenario
            : format === 'appImage'
              ? 'appimage'
              : format === 'deb'
                ? 'deb'
                : 'rpm';
          const preview = lumenPreview(`p${++previewSeq}`, format, effective);
          previews.set(preview.id, preview);
          resolve(preview);
        }, 150);
      });
    },
    async install(previewId, onEvent) {
      const preview = previews.get(previewId);
      if (!preview) throw fail('notFound', 'mock: unknown preview');
      if (
        preview.compat.kind !== 'ok' &&
        !(preview.compat.kind === 'alreadyInstalled' && preview.compat.relation === 'newer')
      ) {
        return { kind: 'failed', code: 'unknown', detail: 'incompatible' };
      }
      const app = lumenApp(
        preview.format === 'appImage' ? 'appImage' : 'systemPackage',
        preview.version
      );
      const run = runScript(
        installScript(preview.format, scenario, { kind: 'installed', app }),
        onEvent
      );
      runs.set(previewId, run);
      const outcome = await run.done;
      runs.delete(previewId);
      if (outcome.kind === 'installed') {
        installed = [outcome.app, ...installed.filter((a) => a.id !== outcome.app.id)];
      }
      return outcome;
    },
    async cancelInstall(previewId) {
      runs.get(previewId)?.cancel({ kind: 'cancelled' });
    },
    onOpenFile(cb) {
      openFileCbs.add(cb);
      return () => {
        openFileCbs.delete(cb);
      };
    }
  };

  const library: LibraryGateway = {
    async listInstalled() {
      return [...installed];
    },
    async uninstall(appId, onEvent) {
      const app = installed.find((a) => a.id === appId);
      if (!app) throw fail('notFound', 'mock: unknown app');
      const run = runScript<UninstallOutcome>(
        uninstallScript(app.source === 'systemPackage', scenario),
        onEvent
      );
      const outcome = await run.done;
      if (outcome.kind === 'removed') installed = installed.filter((a) => a.id !== appId);
      return outcome;
    },
    async launchApp(appId) {
      if (!installed.some((a) => a.id === appId)) throw fail('notFound');
      launched.push(appId);
    }
  };

  const defaults: DefaultsGateway = createMockDefaults(defaultStatus); // lane S13

  const appearanceGw: AppearanceGateway = {
    async get() {
      return appearance;
    },
    onChange(cb) {
      appearanceCbs.add(cb);
      return () => {
        appearanceCbs.delete(cb);
      };
    }
  };

  const windowGw: WindowGateway = {
    async close() {
      windowCalls.push('close');
    },
    async minimize() {
      windowCalls.push('minimize');
    },
    async startDragging() {
      windowCalls.push('startDragging');
    }
  };

  return {
    installer,
    library,
    defaults,
    appearance: appearanceGw,
    window: windowGw,
    controls: {
      scenario,
      emitOpenFile: (path) => openFileCbs.forEach((cb) => cb(path)),
      emitAppearance: (a) => {
        appearance = a;
        appearanceCbs.forEach((cb) => cb(a));
      },
      launched,
      windowCalls,
      defaults: defaultStatus
    }
  };
}
