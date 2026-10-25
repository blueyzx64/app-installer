import { describe, expect, it } from 'vitest';
import {
  installerMachine as m,
  type InstallerEvent as E,
  type InstallerState as S
} from './installer-machine';
import type { InstallPhase, PackagePreview } from './types';

const pv = (): PackagePreview =>
  ({
    id: 'p1',
    name: 'lumen-notes',
    displayName: 'Lumen',
    version: '1',
    scope: 'system',
    needsPermission: true,
    compat: { kind: 'ok' },
    format: 'rpm',
    iconDataUrl: null
  }) as unknown as PackagePreview;
const run = (s: S, ...es: E[]) => es.reduce(m, s);
const prog = (phase: InstallPhase, percent: number | null, etaSecs: number | null = null): E => ({
  type: 'PROGRESS',
  phase,
  percent,
  etaSecs
});
const ready: S = { tag: 'ready', preview: pv(), queued: [] };
const pct = (s: S) => (s as Extract<S, { tag: 'installing' }>).percent;

describe('review_r7 machine', () => {
  it('busy states ignore every gesture except backend events', () => {
    const gestures: E[] = [
      { type: 'DRAG_START' },
      { type: 'DRAG_CANCEL' },
      { type: 'DROP_ON_FOLDER' },
      { type: 'KEY_INSTALL' },
      { type: 'RETRY' },
      { type: 'CLOSE' },
      { type: 'DRAG_OVER', overFolder: true }
    ];
    const starting = run(ready, { type: 'KEY_INSTALL' });
    const awaiting = run(starting, prog('waitingForPermission', null));
    const installing = run(starting, prog('downloading', 10));
    for (const s of [starting, awaiting, installing])
      for (const g of gestures) expect(m(s, g), `${s.tag}+${g.type}`).toBe(s);
  });
  it('OPEN while busy queues FIFO and keeps preview', () => {
    const s = run(
      ready,
      { type: 'KEY_INSTALL' },
      prog('installing', 5),
      { type: 'OPEN', path: 'a' },
      { type: 'OPEN', path: 'b' }
    );
    expect(s).toMatchObject({ tag: 'installing', queued: ['a', 'b'], preview: { id: 'p1' } });
  });
  it('percent never decreases in a phase, 101 -> null', () => {
    const s = run(ready, { type: 'KEY_INSTALL' }, prog('downloading', 50), prog('downloading', 30));
    expect(pct(s)).toBe(50);
    expect(pct(run(s, prog('downloading', 101)))).toBeNull();
  });
  // FINDING: unknown (101) wipes the high-water mark; next real value can be lower than before.
  it('percent: 60 -> 101 -> 55 must not show 55 again', () => {
    const s = run(
      ready,
      { type: 'KEY_INSTALL' },
      prog('downloading', 60),
      prog('downloading', 101),
      prog('downloading', 55)
    );
    expect(pct(s) === null || pct(s)! >= 60).toBe(true);
  });
  // FINDING: late/out-of-order phase event moves finishing back to an earlier phase.
  it('phase never regresses (finishing -> downloading)', () => {
    const s = run(
      ready,
      { type: 'KEY_INSTALL' },
      prog('installing', 100),
      prog('finishing', 100),
      prog('downloading', 20)
    );
    expect((s as Extract<S, { tag: 'installing' }>).phase).toBe('finishing');
  });
  // FINDING: second OPEN during inspecting silently drops the first file.
  it('OPEN while inspecting does not lose the earlier path', () => {
    const s = run({ tag: 'idle' }, { type: 'OPEN', path: 'a' }, { type: 'OPEN', path: 'b' });
    const x = s as Extract<S, { tag: 'inspecting' }>;
    expect([x.path, ...x.queued]).toContain('a');
  });
  it('RETRY after failure re-uses preview; stale INSPECTED ignored', () => {
    const f = run(
      ready,
      { type: 'KEY_INSTALL' },
      { type: 'FINISHED', outcome: { kind: 'failed', code: 'offline', detail: null } }
    );
    expect(m(f, { type: 'RETRY' })).toMatchObject({ tag: 'ready', preview: { id: 'p1' } });
    const i = run({ tag: 'idle' }, { type: 'OPEN', path: 'b' });
    expect(m(i, { type: 'INSPECTED', path: 'a', preview: pv() })).toBe(i);
  });
  it('late events after settle are ignored', () => {
    const done = run(
      ready,
      { type: 'KEY_INSTALL' },
      { type: 'FINISHED', outcome: { kind: 'cancelled' } }
    );
    expect(done.tag).toBe('ready');
    expect(m(done, prog('downloading', 3))).toBe(done);
    expect(m(done, { type: 'FINISHED', outcome: { kind: 'cancelled' } })).toBe(done);
  });
});
