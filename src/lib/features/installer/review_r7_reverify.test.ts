// @vitest-environment jsdom
// R7 re-verify: attack the S5/S6 fixes (peak, phase rank, allowCancel, queue dedupe, Esc, noApp).
import { fireEvent, render, screen } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Controllers } from '$lib/app/context';
import { errorCopy, text } from '$lib/core/copy/copy';
import {
  installerMachine as m,
  type InstallerEvent as E,
  type InstallerState as S
} from '$lib/core/domain/installer-machine';
import type { InstallPhase, PackagePreview } from '$lib/core/domain/types';
import { createMockGateways, type Scenario } from '$lib/infrastructure/mock';
import Host from './review-r7-host.svelte';

const pv = {
  id: 'p1',
  displayName: 'Lumen',
  scope: 'system',
  compat: { kind: 'ok' }
} as unknown as PackagePreview;
const ready: S = { tag: 'ready', preview: pv, queued: [] };
const run = (s: S, ...es: E[]) => es.reduce(m, s);
const prog = (phase: InstallPhase, percent: number | null, allowCancel = false): E => ({
  type: 'PROGRESS',
  phase,
  percent,
  etaSecs: null,
  allowCancel
});
type Inst = Extract<S, { tag: 'installing' }>;

describe('review_r7 reverify machine', () => {
  it('peak does not leak into the next phase', () => {
    const s = run(
      ready,
      { type: 'KEY_INSTALL' },
      prog('downloading', 80),
      prog('installing', 10)
    ) as Inst;
    expect(s.percent).toBe(10);
  });
  it('peak survives several unknowns and a null first report', () => {
    const s = run(
      ready,
      { type: 'KEY_INSTALL' },
      prog('downloading', 101),
      prog('downloading', 40),
      prog('downloading', null),
      prog('downloading', 101),
      prog('downloading', 20)
    ) as Inst;
    expect(s.percent).toBe(40);
  });
  it('allowCancel follows the backend within a phase, never in finishing', () => {
    const a = run(ready, { type: 'KEY_INSTALL' }, prog('downloading', 10, true));
    expect(a.tag === 'installing' && a.allowCancel).toBe(true);
    const b = m(a, prog('downloading', 20, false)) as Inst;
    expect(b.allowCancel).toBe(false);
    const c = run(b, prog('finishing', 100, true));
    expect(c.tag).toBe('installing');
  });
  // NEW FINDING: dedupe only happens while inspecting. The same file opened twice while
  // installing is queued twice, so after Done it is opened again (and again).
  it('same file opened twice while installing is queued once', () => {
    const s = run(
      ready,
      { type: 'KEY_INSTALL' },
      prog('installing', 5),
      { type: 'OPEN', path: '/b.rpm' },
      { type: 'OPEN', path: '/b.rpm' }
    ) as Inst;
    expect(s.queued).toEqual(['/b.rpm']);
  });
  it('same file opened twice while inspecting is not doubled', () => {
    const s = run(
      { tag: 'idle' },
      { type: 'OPEN', path: 'a' },
      { type: 'OPEN', path: 'b' },
      { type: 'OPEN', path: 'b' },
      { type: 'OPEN', path: 'a' }
    );
    expect(s).toMatchObject({ tag: 'inspecting', path: 'a', queued: ['b'] });
  });
  it('noApp copy used for every app-param error when the name is missing', () => {
    for (const c of [
      'offline',
      'missingDependencies',
      'conflicts',
      'untrustedSource',
      'notFound',
      'unknown'
    ] as const) {
      expect(text(errorCopy(c, { app: '' }).message)).not.toMatch(/\{app\}|\.noApp|^ /);
    }
  });
});

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());
const settle = (ms: number) => vi.advanceTimersByTimeAsync(ms);
async function mount(scenario: Scenario) {
  const gw = createMockGateways({ scenario, launchFile: null });
  let ctl!: Controllers;
  render(Host, { gateways: gw, onready: (c: unknown) => (ctl = c as Controllers) });
  return { gw, ctl };
}

describe('review_r7 reverify route', () => {
  it('F-28: Cancel during download -> back to ready, no error, then hidden once installing', async () => {
    const { ctl } = await mount('rpm');
    ctl.installer.open('/x.rpm');
    await settle(300);
    ctl.installer.keyInstall();
    await settle(2600);
    expect(ctl.installer.state).toMatchObject({ tag: 'installing', phase: 'downloading' });
    const btn = screen.getByRole('button', { name: /cancel/i });
    await fireEvent.click(btn);
    await settle(50);
    expect(ctl.installer.state.tag).toBe('ready');
    expect(ctl.installer.error).toBeNull();
  });

  it('F-28: no Cancel button once installing phase starts (allowCancel false)', async () => {
    const { ctl } = await mount('rpm');
    ctl.installer.open('/x.rpm');
    await settle(300);
    ctl.installer.keyInstall();
    await settle(3600);
    expect(ctl.installer.state).toMatchObject({ tag: 'installing', phase: 'installing' });
    expect(screen.queryByRole('button', { name: /cancel/i })).toBeNull();
  });

  it('Escape pressed on the focused Close button closes once (no double close)', async () => {
    const { gw, ctl } = await mount('rpm');
    ctl.installer.open('/x.rpm');
    await settle(300);
    vi.spyOn(gw.installer, 'install').mockResolvedValue({
      kind: 'failed',
      code: 'busy',
      detail: null
    });
    ctl.installer.keyInstall();
    await settle(50);
    expect(ctl.installer.state.tag).toBe('failed');
    const focused = document.activeElement as HTMLElement;
    expect(focused.tagName).toBe('BUTTON');
    await fireEvent.keyDown(focused, { key: 'Escape' });
    await settle(10);
    expect(gw.controls.windowCalls.filter((c) => c === 'close')).toHaveLength(1);
  });

  it('Escape on failed with two queued files advances by exactly one', async () => {
    const { gw, ctl } = await mount('rpm');
    ctl.installer.open('/x.rpm');
    await settle(300);
    vi.spyOn(gw.installer, 'install').mockImplementation(async () => {
      ctl.installer.open('/a.rpm');
      ctl.installer.open('/b.rpm');
      return { kind: 'failed', code: 'unknown', detail: null };
    });
    ctl.installer.keyInstall();
    await settle(50);
    await fireEvent.keyDown(document.activeElement as HTMLElement, { key: 'Escape' });
    expect(ctl.installer.state).toMatchObject({
      tag: 'inspecting',
      path: '/a.rpm',
      queued: ['/b.rpm']
    });
  });

  it('queue toast: shown for a new file while inspecting, not for the same file', async () => {
    const { ctl } = await mount('rpm');
    ctl.installer.open('/x.rpm');
    ctl.installer.open('/x.rpm');
    expect(ctl.installer.queueNotice).toBeNull();
    ctl.installer.open('/y.rpm');
    expect(ctl.installer.queueNotice).not.toBeNull();
  });
});
