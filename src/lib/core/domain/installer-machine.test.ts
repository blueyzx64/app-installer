import { describe, expect, it } from 'vitest';
import {
  canCancel,
  initialInstallerState,
  installerMachine as m,
  isBusy,
  isInstallable,
  isUpdate,
  normalizePercent,
  type InstallerEvent,
  type InstallerState
} from './installer-machine';
import type { InstalledApp, PackagePreview } from './types';

const preview = (compat: PackagePreview['compat'] = { kind: 'ok' }): PackagePreview => ({
  id: 'p1',
  name: 'lumen-notes',
  format: 'rpm',
  displayName: 'Lumen Notes',
  version: '2.4.1',
  summary: null,
  iconDataUrl: null,
  sizeBytes: 1,
  scope: 'system',
  needsPermission: true,
  compat
});
const app: InstalledApp = {
  id: 'lumen',
  displayName: 'Lumen Notes',
  version: '2.4.1',
  iconDataUrl: null,
  source: 'systemPackage',
  installedAt: '2026-10-25T00:00:00Z',
  canLaunch: true
};
const P = preview();
const ready: InstallerState = { tag: 'ready', preview: P, queued: [] };
const dragging: InstallerState = { tag: 'dragging', preview: P, overFolder: false, queued: [] };
const starting: InstallerState = { tag: 'starting', preview: P, queued: [] };
const awaiting: InstallerState = { tag: 'awaitingPermission', preview: P, queued: [] };
const installing = (
  percent: number | null = 10,
  phase: 'downloading' | 'installing' = 'downloading'
) =>
  ({
    tag: 'installing',
    preview: P,
    phase,
    percent,
    peak: percent ?? 0,
    etaSecs: 5,
    allowCancel: phase === 'downloading',
    queued: []
  }) as InstallerState;
const succeeded: InstallerState = { tag: 'succeeded', preview: P, app, queued: [] };
const incompatible: InstallerState = {
  tag: 'incompatible',
  preview: preview({ kind: 'wrongArch', packageArch: 'a', hostArch: 'b' }),
  queued: []
};
const failed: InstallerState = {
  tag: 'failed',
  preview: P,
  path: null,
  code: 'offline',
  detail: null,
  queued: []
};
const inspecting: InstallerState = { tag: 'inspecting', path: '/a.rpm', queued: [] };
const prog = (
  phase: 'preparing' | 'waitingForPermission' | 'downloading' | 'installing' | 'finishing',
  percent: number | null = null
): InstallerEvent => ({ type: 'PROGRESS', phase, percent, etaSecs: null });

describe('installerMachine · inspection', () => {
  it('idle + OPEN -> inspecting', () => {
    expect(m(initialInstallerState, { type: 'OPEN', path: '/a.rpm' })).toEqual(inspecting);
  });
  it('inspecting + INSPECTED(ok) -> ready', () => {
    expect(m(inspecting, { type: 'INSPECTED', preview: P })).toEqual({
      ...ready,
      source: inspecting.path
    });
  });
  it.each([
    { kind: 'wrongFamily', madeFor: 'debBased' },
    { kind: 'wrongArch', packageArch: 'a', hostArch: 'b' },
    { kind: 'alreadyInstalled', relation: 'same', installedVersion: '1' },
    { kind: 'alreadyInstalled', relation: 'older', installedVersion: '9' },
    { kind: 'missingRuntime', what: 'fuse2' },
    { kind: 'unsupported', reason: 'dwarfs' }
  ] as PackagePreview['compat'][])('INSPECTED(%j) -> incompatible', (compat) => {
    const s = m(inspecting, { type: 'INSPECTED', preview: preview(compat) });
    expect(s.tag).toBe('incompatible');
  });
  it('alreadyInstalled.newer is Update: stays ready', () => {
    const p = preview({ kind: 'alreadyInstalled', relation: 'newer', installedVersion: '1' });
    expect(m(inspecting, { type: 'INSPECTED', preview: p }).tag).toBe('ready');
    expect(isUpdate(p)).toBe(true);
    expect(isInstallable(p)).toBe(true);
    expect(isUpdate(P)).toBe(false);
    expect(isUpdate(null)).toBe(false);
  });
  it('inspecting + INSPECT_FAILED -> failed (no preview)', () => {
    expect(m(inspecting, { type: 'INSPECT_FAILED', code: 'corruptFile', detail: 'x' })).toEqual({
      tag: 'failed',
      preview: null,
      path: '/a.rpm',
      code: 'corruptFile',
      detail: 'x',
      queued: []
    });
  });
  it('ignores stale inspect results for another path', () => {
    expect(m(inspecting, { type: 'INSPECTED', path: '/other', preview: P })).toBe(inspecting);
    expect(
      m(inspecting, { type: 'INSPECT_FAILED', path: '/other', code: 'unknown', detail: null })
    ).toBe(inspecting);
  });
  it('ready + OPEN(other) -> inspecting', () => {
    expect(m(ready, { type: 'OPEN', path: '/b' })).toEqual({
      tag: 'inspecting',
      path: '/b',
      queued: []
    });
  });
  it.each([incompatible, failed, succeeded, inspecting])('OPEN from %j starts inspecting', (s) => {
    expect(m(s, { type: 'OPEN', path: '/n' }).tag).toBe('inspecting');
  });
  it('RETRY on inspect failure re-inspects the same path', () => {
    const f = m(inspecting, { type: 'INSPECT_FAILED', code: 'unknown', detail: null });
    expect(m(f, { type: 'RETRY' })).toEqual(inspecting);
  });
});

describe('installerMachine · drag and start', () => {
  it('ready -> dragging -> ready on cancel (spring back)', () => {
    const d = m(ready, { type: 'DRAG_START' });
    expect(d).toEqual(dragging);
    expect(m(d, { type: 'DRAG_OVER', overFolder: true })).toMatchObject({ overFolder: true });
    expect(m(d, { type: 'DRAG_CANCEL' })).toEqual(ready);
  });
  it('dragging + DROP_ON_FOLDER -> starting', () => {
    expect(m(dragging, { type: 'DROP_ON_FOLDER' })).toEqual(starting);
  });
  it('ready + KEY_INSTALL -> starting', () => {
    expect(m(ready, { type: 'KEY_INSTALL' })).toEqual(starting);
  });
  it('drag events outside dragging/ready are ignored', () => {
    expect(m(ready, { type: 'DRAG_OVER', overFolder: true })).toBe(ready);
    expect(m(ready, { type: 'DROP_ON_FOLDER' })).toBe(ready);
    expect(m(dragging, { type: 'DRAG_START' })).toBe(dragging);
  });
});

describe('installerMachine · progress', () => {
  it('starting + PROGRESS(waitingForPermission) -> awaitingPermission', () => {
    expect(m(starting, prog('waitingForPermission')).tag).toBe('awaitingPermission');
  });
  it.each(['preparing', 'downloading', 'installing', 'finishing'] as const)(
    'starting + PROGRESS(%s) -> installing',
    (phase) => {
      expect(m(starting, prog(phase, 3))).toMatchObject({ tag: 'installing', phase, percent: 3 });
    }
  );
  it.each(['downloading', 'installing', 'finishing', 'preparing'] as const)(
    'awaitingPermission + PROGRESS(%s) -> installing',
    (phase) => {
      expect(m(awaiting, prog(phase, 0)).tag).toBe('installing');
    }
  );
  it('awaitingPermission stays on repeated waitingForPermission', () => {
    expect(m(awaiting, prog('waitingForPermission')).tag).toBe('awaitingPermission');
  });
  it('installing ignores waitingForPermission', () => {
    const s = installing();
    expect(m(s, prog('waitingForPermission'))).toBe(s);
  });
  it('percent never goes backwards within a phase', () => {
    const s = m(installing(50), prog('downloading', 30));
    expect(s).toMatchObject({ percent: 50 });
  });
  it('percent may restart on a new phase', () => {
    expect(m(installing(90), prog('installing', 5))).toMatchObject({
      phase: 'installing',
      percent: 5
    });
  });
  it('clamps percent to 0..100 and 101 (unknown) becomes null', () => {
    expect(m(starting, prog('downloading', -5))).toMatchObject({ percent: 0 });
    expect(m(starting, prog('downloading', 101))).toMatchObject({ percent: null });
    expect(m(installing(40), prog('downloading', 101))).toMatchObject({ percent: null });
    expect(normalizePercent(NaN)).toBeNull();
    expect(normalizePercent(100)).toBe(100);
  });
  it('plan is kept through starting -> installing', () => {
    const plan = { extraPackages: 2, downloadBytes: 9 };
    const s1 = m(starting, { type: 'PLAN', plan });
    const s2 = m(s1, prog('downloading', 0));
    expect(s2).toMatchObject({ tag: 'installing', plan });
    expect(m(s2, { type: 'PLAN', plan: { extraPackages: 3, downloadBytes: 1 } })).toMatchObject({
      plan: { extraPackages: 3 }
    });
  });
  it('plan survives awaitingPermission', () => {
    const plan = { extraPackages: 1, downloadBytes: 1 };
    const a = m(m(starting, { type: 'PLAN', plan }), prog('waitingForPermission'));
    expect(a).toMatchObject({ tag: 'awaitingPermission', plan });
    expect(m(a, prog('installing', 1))).toMatchObject({ plan });
  });
  it('eta is passed through', () => {
    expect(
      m(starting, { type: 'PROGRESS', phase: 'downloading', percent: 1, etaSecs: 42 })
    ).toMatchObject({
      etaSecs: 42
    });
  });
});

describe('installerMachine · outcomes', () => {
  const done = { type: 'FINISHED', outcome: { kind: 'installed', app } } as const;
  it.each([starting, awaiting, installing()])('FINISHED(installed) from %j -> succeeded', (s) => {
    expect(m(s, done)).toEqual(succeeded);
  });
  it.each([starting, awaiting, installing()])(
    'FINISHED(cancelled) from %j -> ready (no error)',
    (s) => {
      expect(m(s, { type: 'FINISHED', outcome: { kind: 'cancelled' } })).toEqual(ready);
    }
  );
  it.each([starting, awaiting, installing()])('FINISHED(failed) from %j -> failed', (s) => {
    expect(
      m(s, { type: 'FINISHED', outcome: { kind: 'failed', code: 'offline', detail: 'd' } })
    ).toEqual({ ...failed, detail: 'd' });
  });
  it('FINISHED is ignored outside busy states', () => {
    for (const s of [ready, dragging, succeeded, failed, incompatible, inspecting]) {
      expect(m(s, done)).toBe(s);
    }
  });
  it('failed + RETRY -> ready; queue kept', () => {
    expect(m({ ...failed, queued: ['/q'] }, { type: 'RETRY' })).toEqual({
      ...ready,
      queued: ['/q']
    });
  });
  it('RETRY is ignored elsewhere', () => {
    expect(m(ready, { type: 'RETRY' })).toBe(ready);
    expect(m(succeeded, { type: 'RETRY' })).toBe(succeeded);
  });
});

describe('installerMachine · close and queue', () => {
  it.each([ready, succeeded, incompatible, failed])('CLOSE from %j -> idle', (s) => {
    expect(m(s, { type: 'CLOSE' })).toEqual({ tag: 'idle' });
  });
  it('CLOSE opens the next queued package', () => {
    const s = m({ ...succeeded, queued: ['/a', '/b'] }, { type: 'CLOSE' });
    expect(s).toEqual({ tag: 'inspecting', path: '/a', queued: ['/b'] });
  });
  it('idle ignores everything but OPEN', () => {
    const idle = initialInstallerState;
    expect(m(idle, { type: 'CLOSE' })).toBe(idle);
    expect(m(idle, { type: 'KEY_INSTALL' })).toBe(idle);
  });
});

describe('installerMachine · invariants', () => {
  const busyStates: InstallerState[] = [starting, awaiting, installing()];
  const gestures: InstallerEvent[] = [
    { type: 'DRAG_START' },
    { type: 'DRAG_OVER', overFolder: true },
    { type: 'DRAG_CANCEL' },
    { type: 'DROP_ON_FOLDER' },
    { type: 'KEY_INSTALL' },
    { type: 'RETRY' },
    { type: 'CLOSE' },
    { type: 'INSPECTED', preview: P },
    { type: 'INSPECT_FAILED', code: 'unknown', detail: null }
  ];
  it('no user gesture leaves a busy state (only backend outcomes do)', () => {
    for (const s of busyStates) {
      for (const g of gestures) expect(m(s, g)).toBe(s);
    }
  });
  it('OPEN while busy or dragging queues and never replaces the preview', () => {
    for (const s of [...busyStates, dragging]) {
      const n = m(s, { type: 'OPEN', path: '/q1' });
      expect(n.tag).toBe(s.tag);
      expect(n).toMatchObject({ preview: P, queued: ['/q1'] });
      const n2 = m(n, { type: 'OPEN', path: '/q2' });
      expect(n2).toMatchObject({ queued: ['/q1', '/q2'] });
    }
  });
  it('queue survives cancelled outcome', () => {
    const s = m(
      { ...awaiting, queued: ['/q'] },
      { type: 'FINISHED', outcome: { kind: 'cancelled' } }
    );
    expect(s).toMatchObject({ tag: 'ready', queued: ['/q'] });
  });
  it('isBusy / canCancel', () => {
    expect(busyStates.every(isBusy)).toBe(true);
    expect(isBusy(ready)).toBe(false);
    expect(canCancel(awaiting)).toBe(true);
    expect(canCancel(starting)).toBe(true);
    expect(canCancel(installing(1, 'downloading'))).toBe(true);
    expect(canCancel({ ...(installing() as object), phase: 'finishing' } as InstallerState)).toBe(
      false
    );
    expect(canCancel(ready)).toBe(false);
  });
  it('is pure: input state not mutated', () => {
    const s = installing(10);
    const copy = structuredClone(s);
    m(s, prog('downloading', 80));
    expect(s).toEqual(copy);
  });
});

describe('installerMachine · no double opens', () => {
  const run = (s: InstallerState, ...es: Parameters<typeof m>[1][]) => es.reduce(m, s);
  const fromFile = run(inspecting, { type: 'INSPECTED', preview: P });
  const queueOf = (s: InstallerState) => (s.tag === 'idle' ? [] : s.queued);

  it('the source file is kept through drag, install and outcome', () => {
    const s = run(fromFile, { type: 'KEY_INSTALL' }, prog('downloading', 5));
    expect(s).toMatchObject({ tag: 'installing', source: '/a.rpm' });
    const done = m(s, { type: 'FINISHED', outcome: { kind: 'cancelled' } });
    expect(done).toMatchObject({ tag: 'ready', source: '/a.rpm' });
  });
  it.each(['dragging', 'starting', 'awaitingPermission', 'installing'] as const)(
    'OPEN of the file on screen or an already queued file does not queue (%s)',
    (tag) => {
      const busy =
        tag === 'dragging'
          ? m(fromFile, { type: 'DRAG_START' })
          : tag === 'starting'
            ? m(fromFile, { type: 'KEY_INSTALL' })
            : tag === 'awaitingPermission'
              ? run(fromFile, { type: 'KEY_INSTALL' }, prog('waitingForPermission', null))
              : run(fromFile, { type: 'KEY_INSTALL' }, prog('installing', 5));
      expect(busy.tag).toBe(tag);
      const s = run(
        busy,
        { type: 'OPEN', path: '/a.rpm' },
        { type: 'OPEN', path: '/b.rpm' },
        { type: 'OPEN', path: '/b.rpm' }
      );
      expect(queueOf(s)).toEqual(['/b.rpm']);
    }
  );
  it('opening a queued file directly takes it out of the queue', () => {
    const s = run(
      fromFile,
      { type: 'KEY_INSTALL' },
      prog('installing', 5),
      { type: 'OPEN', path: '/b.rpm' },
      { type: 'FINISHED', outcome: { kind: 'failed', code: 'offline', detail: null } },
      { type: 'OPEN', path: '/b.rpm' }
    );
    expect(s).toEqual({ tag: 'inspecting', path: '/b.rpm', queued: [] });
  });
});
