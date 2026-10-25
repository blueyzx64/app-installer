// Timeline for a fake install/uninstall, ported from the prototype's timers.
// Pure data: the gateway schedules it with setTimeout so vi fake timers drive it.
import type {
  ErrorCode,
  InstallEvent,
  InstallOutcome,
  InstallPhase,
  PackageFormat,
  UninstallOutcome
} from '../../core/domain/types';
import type { Scenario } from './scenario';

export interface ScriptStep<O> {
  /** ms since start. */
  at: number;
  event?: InstallEvent;
  outcome?: O;
}

function progress(
  at: number,
  phase: InstallPhase,
  percent: number | null,
  etaSecs: number | null
): ScriptStep<never> {
  return { at, event: { kind: 'progress', phase, percent, etaSecs } };
}

/** `count` evenly spaced progress steps for one phase, 100/count .. 100 percent. */
function ramp(
  start: number,
  every: number,
  phase: InstallPhase,
  count: number,
  etaTotalSecs: number
): ScriptStep<never>[] {
  const out: ScriptStep<never>[] = [];
  for (let k = 1; k <= count; k++) {
    const percent = Math.round((100 * k) / count);
    out.push(
      progress(start + k * every, phase, percent, Math.ceil(etaTotalSecs * (1 - k / count)))
    );
  }
  return out;
}

export function installScript(
  format: PackageFormat,
  scenario: Scenario,
  done: InstallOutcome
): ScriptStep<InstallOutcome>[] {
  if (format === 'appImage' && scenario === 'null-percent') {
    // Amount unknown: the bar stays indeterminate until the last step.
    return [
      progress(0, 'preparing', null, null),
      progress(400, 'installing', null, null),
      progress(1200, 'installing', null, null),
      progress(2200, 'finishing', 100, null),
      { at: 2600, outcome: done }
    ];
  }
  if (format === 'appImage') {
    return [
      progress(0, 'preparing', 0, null),
      ...ramp(300, 250, 'installing', 8, 2),
      progress(2500, 'finishing', 100, null),
      { at: 2900, outcome: done }
    ];
  }
  // Password prompt comes first, so the window can show the permission shade straight away.
  const steps: ScriptStep<InstallOutcome>[] = [progress(0, 'waitingForPermission', null, null)];
  if (scenario === 'cancel-permission') {
    steps.push({ at: 1600, outcome: { kind: 'cancelled' } });
    return steps;
  }
  steps.push(
    progress(1000, 'preparing', 0, null),
    { at: 1500, event: { kind: 'plan', extraPackages: 2, downloadBytes: 62_000_000 } },
    progress(1500, 'downloading', 0, 6)
  );
  if (scenario === 'fail-offline') {
    steps.push(progress(1900, 'downloading', 20, 5), progress(2300, 'downloading', 40, null), {
      at: 2800,
      outcome: { kind: 'failed', code: 'offline', detail: 'mock: could not reach the network' }
    });
    return steps;
  }
  steps.push(
    ...ramp(1500, 300, 'downloading', 5, 3),
    ...ramp(3100, 250, 'installing', 10, 3),
    progress(5700, 'finishing', 100, null),
    { at: 6100, outcome: done }
  );
  return steps;
}

export function uninstallScript(
  system: boolean,
  scenario: Scenario
): ScriptStep<UninstallOutcome>[] {
  if (!system) {
    return [...ramp(0, 150, 'installing', 4, 1), { at: 900, outcome: { kind: 'removed' } }];
  }
  const steps: ScriptStep<UninstallOutcome>[] = [progress(0, 'waitingForPermission', null, null)];
  if (scenario === 'cancel-permission') {
    steps.push({ at: 1200, outcome: { kind: 'cancelled' } });
    return steps;
  }
  if (scenario === 'fail-offline') {
    const code: ErrorCode = 'unknown';
    steps.push({ at: 1200, outcome: { kind: 'failed', code, detail: 'mock: removal failed' } });
    return steps;
  }
  steps.push(...ramp(1000, 200, 'installing', 5, 2), { at: 2200, outcome: { kind: 'removed' } });
  return steps;
}
