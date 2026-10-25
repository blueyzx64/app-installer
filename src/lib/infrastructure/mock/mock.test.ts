import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { InstallEvent } from '../../core/domain/types';
import { createMockGateways, parseScenario, SCENARIOS } from './index';

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

async function open(scenario: (typeof SCENARIOS)[number], path = '/x.rpm') {
  const g = createMockGateways({ scenario, launchFile: null });
  const p = g.installer.openPackage(path);
  await vi.advanceTimersByTimeAsync(200);
  return { g, preview: await p };
}

describe('parseScenario', () => {
  it('reads known scenarios only', () => {
    expect(parseScenario('?scenario=deb')).toBe('deb');
    expect(parseScenario('?scenario=nope')).toBeNull();
    expect(parseScenario('')).toBeNull();
  });
});

describe('mock installer · previews', () => {
  it.each([
    ['rpm', 'rpm', 'system', { kind: 'ok' }],
    ['deb', 'deb', 'system', { kind: 'ok' }],
    ['appimage', 'appImage', 'user', { kind: 'ok' }],
    ['incompatible-arch', 'rpm', 'system', { kind: 'wrongArch' }],
    ['already-installed', 'rpm', 'system', { kind: 'alreadyInstalled', relation: 'same' }],
    ['update', 'rpm', 'system', { kind: 'alreadyInstalled', relation: 'newer' }],
    ['missing-fuse2', 'appImage', 'user', { kind: 'missingRuntime', what: 'fuse2' }]
  ] as const)('%s', async (scenario, format, scope, compat) => {
    const { preview } = await open(scenario);
    expect(preview).toMatchObject({ format, scope, compat, displayName: 'Lumen Notes' });
    expect(preview.id).toMatch(/^p\d+$/);
    expect(preview.needsPermission).toBe(scope === 'system');
  });
  it('without a scenario, the file extension picks the format', async () => {
    const g = createMockGateways({ launchFile: null });
    const p = g.installer.openPackage('/a/Lumen.AppImage');
    await vi.advanceTimersByTimeAsync(200);
    expect(await p).toMatchObject({ format: 'appImage', scope: 'user' });
  });
  it('launch file is consumed once', async () => {
    const g = createMockGateways({ scenario: 'deb' });
    expect(await g.installer.takeLaunchFile()).toMatch(/\.deb$/);
    expect(await g.installer.takeLaunchFile()).toBeNull();
  });
  it('empty-library has no launch file', async () => {
    const g = createMockGateways({ scenario: 'empty-library' });
    expect(await g.installer.takeLaunchFile()).toBeNull();
    expect(await g.library.listInstalled()).toEqual([]);
  });
  it('onOpenFile subscribes and unsubscribes', () => {
    const g = createMockGateways({ scenario: 'rpm' });
    const cb = vi.fn();
    const off = g.installer.onOpenFile(cb);
    g.controls.emitOpenFile('/a');
    off();
    g.controls.emitOpenFile('/b');
    expect(cb).toHaveBeenCalledOnce();
    expect(cb).toHaveBeenCalledWith('/a');
  });
});

describe('mock installer · install', () => {
  async function run(scenario: (typeof SCENARIOS)[number], path?: string) {
    const { g, preview } = await open(scenario, path);
    const events: InstallEvent[] = [];
    const out = g.installer.install(preview.id, (e) => events.push(e));
    await vi.advanceTimersByTimeAsync(10_000);
    return { g, preview, events, outcome: await out };
  }
  it('rpm: asks permission, plans a download, installs, adds to the library', async () => {
    const { g, events, outcome } = await run('rpm');
    const phases = events.flatMap((e) => (e.kind === 'progress' ? [e.phase] : []));
    expect(phases[0]).toBe('waitingForPermission');
    expect(phases).toContain('waitingForPermission');
    expect(phases.indexOf('downloading')).toBeLessThan(phases.indexOf('installing'));
    expect(phases.at(-1)).toBe('finishing');
    expect(events.some((e) => e.kind === 'plan')).toBe(true);
    expect(outcome.kind).toBe('installed');
    expect((await g.library.listInstalled()).some((a) => a.id === 'lumen-notes')).toBe(true);
  });
  it('appimage: no permission step', async () => {
    const { events, outcome } = await run('appimage');
    expect(events.some((e) => e.kind === 'progress' && e.phase === 'waitingForPermission')).toBe(
      false
    );
    expect(outcome).toMatchObject({ kind: 'installed', app: { source: 'appImage' } });
  });
  it('fail-offline fails with the offline code', async () => {
    const { outcome } = await run('fail-offline');
    expect(outcome).toMatchObject({ kind: 'failed', code: 'offline' });
  });
  it('cancel-permission ends cancelled, not failed', async () => {
    const { outcome } = await run('cancel-permission');
    expect(outcome).toEqual({ kind: 'cancelled' });
  });
  it('cancelInstall stops the run', async () => {
    const { g, preview } = await open('rpm');
    const events: InstallEvent[] = [];
    const out = g.installer.install(preview.id, (e) => events.push(e));
    await vi.advanceTimersByTimeAsync(600);
    await g.installer.cancelInstall(preview.id);
    expect(await out).toEqual({ kind: 'cancelled' });
    const n = events.length;
    await vi.advanceTimersByTimeAsync(10_000);
    expect(events.length).toBe(n);
  });
  it('progress percent is monotonic within a phase', async () => {
    const { events } = await run('rpm');
    const last: Record<string, number> = {};
    for (const e of events) {
      if (e.kind !== 'progress' || e.percent === null) continue;
      expect(e.percent).toBeGreaterThanOrEqual(last[e.phase] ?? 0);
      last[e.phase] = e.percent;
    }
  });
  it('install of an incompatible preview fails as unknown (D-007)', async () => {
    const { outcome } = await run('incompatible-arch');
    expect(outcome).toMatchObject({ kind: 'failed', code: 'unknown' });
  });
  it('unknown preview rejects with notFound', async () => {
    const g = createMockGateways({ scenario: 'rpm' });
    await expect(g.installer.install('p99', () => {})).rejects.toMatchObject({ code: 'notFound' });
  });
});

describe('mock library', () => {
  it('uninstall removes a user app', async () => {
    const g = createMockGateways({ scenario: 'rpm' });
    const out = g.library.uninstall('pixel-draw', () => {});
    await vi.advanceTimersByTimeAsync(5000);
    expect(await out).toEqual({ kind: 'removed' });
    expect((await g.library.listInstalled()).map((a) => a.id)).not.toContain('pixel-draw');
  });
  it('system uninstall asks permission; cancel-permission cancels and keeps the app', async () => {
    const g = createMockGateways({ scenario: 'cancel-permission' });
    const events: InstallEvent[] = [];
    const out = g.library.uninstall('orbit-player', (e) => events.push(e));
    await vi.advanceTimersByTimeAsync(5000);
    expect(await out).toEqual({ kind: 'cancelled' });
    expect(events[0]).toMatchObject({ phase: 'waitingForPermission' });
    expect((await g.library.listInstalled()).map((a) => a.id)).toContain('orbit-player');
  });
  it('uninstallPlan: system apps have extras and need permission, user apps do not', async () => {
    const g = createMockGateways({ scenario: 'rpm' });
    expect(await g.library.uninstallPlan('orbit-player')).toEqual({
      alsoRemoves: 2,
      needsPermission: true
    });
    expect(await g.library.uninstallPlan('pixel-draw')).toEqual({
      alsoRemoves: 0,
      needsPermission: false
    });
    await expect(g.library.uninstallPlan('zzz')).rejects.toMatchObject({ code: 'notFound' });
  });
  it('fail-offline: system removal fails and keeps the app', async () => {
    const g = createMockGateways({ scenario: 'fail-offline' });
    const out = g.library.uninstall('orbit-player', () => {});
    await vi.advanceTimersByTimeAsync(5000);
    expect(await out).toMatchObject({ kind: 'failed' });
    expect((await g.library.listInstalled()).map((a) => a.id)).toContain('orbit-player');
  });
  it('empty-library lists nothing', async () => {
    const g = createMockGateways({ scenario: 'empty-library' });
    expect(await g.library.listInstalled()).toEqual([]);
  });
  it('launchApp records and rejects unknown ids', async () => {
    const g = createMockGateways({ scenario: 'rpm' });
    await g.library.launchApp('pixel-draw');
    expect(g.controls.launched).toEqual(['pixel-draw']);
    await expect(g.library.launchApp('zzz')).rejects.toMatchObject({ code: 'notFound' });
  });
  it('update scenario replaces the old version on install', async () => {
    const g = createMockGateways({ scenario: 'update' });
    expect((await g.library.listInstalled())[0]).toMatchObject({ version: '2.3.0' });
    const p = g.installer.openPackage('/x.rpm');
    await vi.advanceTimersByTimeAsync(200);
    const pv = await p;
    const out = g.installer.install(pv.id, () => {});
    await vi.advanceTimersByTimeAsync(10_000);
    await out;
    const lumen = (await g.library.listInstalled()).filter((a) => a.id === 'lumen-notes');
    expect(lumen).toHaveLength(1);
    expect(lumen[0]).toMatchObject({ version: '2.4.1' });
  });
});

describe('mock defaults, appearance, window', () => {
  it('defaults flow', async () => {
    const g = createMockGateways({ scenario: 'rpm' });
    expect(await g.defaults.status()).toEqual({ isDefault: false, askedBefore: false });
    await g.defaults.dismissPrompt();
    expect(await g.defaults.status()).toEqual({ isDefault: false, askedBefore: true });
    await g.defaults.makeDefault();
    expect((await g.defaults.status()).isDefault).toBe(true);
  });
  it('appearance emits changes', async () => {
    const g = createMockGateways({ scenario: 'rpm' });
    const cb = vi.fn();
    const off = g.appearance.onChange(cb);
    g.controls.emitAppearance({ ...(await g.appearance.get()), colorScheme: 'dark' });
    off();
    g.controls.emitAppearance({ ...(await g.appearance.get()), colorScheme: 'light' });
    expect(cb).toHaveBeenCalledOnce();
  });
  it('window calls are recorded', async () => {
    const g = createMockGateways({ scenario: 'rpm' });
    await g.window.minimize();
    await g.window.close();
    expect(g.controls.windowCalls).toEqual(['minimize', 'close']);
  });
});
