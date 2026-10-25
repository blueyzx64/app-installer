// @vitest-environment jsdom
import { render, screen } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Controllers } from '$lib/app/context';
import { createMockGateways, type Scenario } from '$lib/infrastructure/mock';
import { InstallerController } from './InstallerController.svelte';
import Host from './review-r7-host.svelte';

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());
const settle = (ms: number) => vi.advanceTimersByTimeAsync(ms);

async function setup(scenario: Scenario, file = '/x.rpm') {
  const gw = createMockGateways({ scenario, launchFile: null });
  const c = new InstallerController(gw);
  c.open(file);
  await settle(300);
  return { gw, c };
}

describe('review_r7 controller', () => {
  it('double drop + Enter starts exactly one install', async () => {
    const { gw, c } = await setup('rpm');
    const spy = vi.spyOn(gw.installer, 'install');
    c.startDrag();
    c.dragOver(true);
    c.endDrag();
    c.endDrag();
    c.keyInstall();
    c.keyInstall();
    expect(spy).toHaveBeenCalledTimes(1);
  });

  it('cancel during starting is delivered once the backend knows the install', async () => {
    const { gw, c } = await setup('rpm');
    const spy = vi.spyOn(gw.installer, 'cancelInstall');
    c.keyInstall();
    c.cancelInstall();
    await settle(50);
    await settle(3000);
    expect(spy).toHaveBeenCalledTimes(1);
    expect(c.state.tag).toBe('ready');
  });

  it('user scope never shows the permission shade, even if backend sends waitingForPermission', async () => {
    const { gw, c } = await setup('appimage', '/x.AppImage');
    vi.spyOn(gw.installer, 'install').mockImplementation(async (_id, onEvent) => {
      onEvent({
        kind: 'progress',
        phase: 'waitingForPermission',
        percent: null,
        etaSecs: null,
        allowCancel: false
      });
      return new Promise(() => {});
    });
    c.keyInstall();
    await settle(10);
    expect(c.waitingForPermission).toBe(false);
    expect(c.phaseText).not.toMatch(/permission/i);
  });

  it('rpm: shade only on waitingForPermission, not on starting/installing', async () => {
    const { c } = await setup('rpm');
    c.keyInstall();
    expect(c.waitingForPermission).toBe(false);
    await settle(100);
    expect(c.waitingForPermission).toBe(true);
    await settle(1800);
    expect(c.state.tag).toBe('installing');
    expect(c.waitingForPermission).toBe(false);
  });

  it('queued file opens after success and after cancel (on Done/Close)', async () => {
    const { gw, c } = await setup('rpm');
    c.keyInstall();
    await settle(2000);
    c.open('/b.rpm');
    expect(c.queuedCount).toBe(1);
    expect(c.state.tag).toBe('installing');
    await settle(10_000);
    expect(c.state.tag).toBe('succeeded');
    c.close();
    expect(c.state.tag).toBe('inspecting');
    expect(gw.controls.windowCalls).not.toContain('close');
  });

  it('events after unmount do not throw (start cleanup)', async () => {
    const { gw, c } = await setup('rpm');
    const off = c.start();
    off();
    gw.controls.emitOpenFile('/late.rpm');
    expect(c.state.tag).toBe('ready');
  });

  it('first-launch file given before /install is mounted is not lost', async () => {
    const gw = createMockGateways({ scenario: 'rpm', launchFile: '/x.rpm' });
    const c = new InstallerController(gw);
    expect(await c.takeLaunch()).toBe(true);
    expect(c.state.tag).toBe('inspecting');
  });
});

describe('review_r7 route', () => {
  async function mount(scenario: Scenario) {
    const gw = createMockGateways({ scenario, launchFile: null });
    let ctl!: Controllers;
    render(Host, { gateways: gw, onready: (c: unknown) => (ctl = c as Controllers) });
    return { gw, ctl };
  }

  // FINDING F-28: Cancel button must show while backend allowCancel is true (download phase).
  it('F-28: download phase offers a Cancel button', async () => {
    const { ctl } = await mount('rpm');
    ctl.installer.open('/x.rpm');
    await settle(300);
    ctl.installer.keyInstall();
    await settle(2600);
    expect(ctl.installer.state.tag).toBe('installing');
    expect(ctl.installer.canCancel).toBe(true);
    expect(screen.queryByRole('button', { name: /cancel/i })).not.toBeNull();
  });

  // FINDING: /install with no package renders an empty page.
  it('/install with nothing opened renders something', async () => {
    const { ctl } = await mount('rpm');
    expect(ctl.installer.state.tag).toBe('idle');
    expect(document.querySelector('main.install')!.textContent!.trim()).not.toBe('');
  });

  // FINDING: error.busy has no action -> alert without any button.
  it('busy failure shows at least one button', async () => {
    const { gw, ctl } = await mount('rpm');
    vi.spyOn(gw.installer, 'install').mockResolvedValue({
      kind: 'failed',
      code: 'busy',
      detail: null
    });
    ctl.installer.open('/x.rpm');
    await settle(300);
    ctl.installer.keyInstall();
    await settle(50);
    expect(ctl.installer.state.tag).toBe('failed');
    expect(screen.queryAllByRole('button').length).toBeGreaterThan(0);
  });
});
