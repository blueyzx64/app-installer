import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createMockGateways, type Scenario } from '$lib/infrastructure/mock';
import { InstallerController, OPEN_APP_CLOSE_MS } from './InstallerController.svelte';

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

async function setup(scenario: Scenario) {
  const gw = createMockGateways({ scenario, launchFile: null });
  const c = new InstallerController(gw);
  c.open('/x.rpm');
  await vi.advanceTimersByTimeAsync(300);
  return { gw, c };
}
const settle = (ms = 10_000) => vi.advanceTimersByTimeAsync(ms);

describe('InstallerController', () => {
  it('opens a package and becomes ready', async () => {
    const { c } = await setup('rpm');
    expect(c.state.tag).toBe('ready');
    expect(c.appName).toBe('Lumen Notes');
    expect(c.isUpdate).toBe(false);
    expect(c.busy).toBe(false);
  });

  it('drag onto the folder installs end to end (rpm)', async () => {
    const { c, gw } = await setup('rpm');
    c.startDrag();
    expect(c.state.tag).toBe('dragging');
    c.dragOver(true);
    expect(c.dragOverFolder).toBe(true);
    c.endDrag();
    expect(c.state.tag).toBe('starting');
    expect(c.busy).toBe(true);

    await settle(700);
    expect(c.state.tag).toBe('awaitingPermission');
    expect(c.phaseText).toBe('Waiting for permission…');
    expect(c.canCancel).toBe(true);

    await settle(1800);
    expect(c.state.tag).toBe('installing');
    expect(c.phaseText).toBe('Getting a few things it needs (62 MB)');

    await settle();
    expect(c.state.tag).toBe('succeeded');
    expect((await gw.library.listInstalled()).some((a) => a.id === 'lumen-notes')).toBe(true);
  });

  it('shows percent, indeterminate null before progress, and eta text', async () => {
    const { c } = await setup('appimage');
    c.keyInstall();
    await settle(1000);
    expect(c.state.tag).toBe('installing');
    expect(c.percent).not.toBeNull();
    expect(c.percent!).toBeGreaterThan(0);
    expect(c.etaText).toBe('A few seconds left');
    expect(c.phaseText).toBe('Installing Lumen Notes…');
  });

  it('releasing away from the folder springs back', async () => {
    const { c } = await setup('rpm');
    c.startDrag();
    c.dragOver(false);
    c.endDrag();
    expect(c.state.tag).toBe('ready');
  });

  it('cancel at the password prompt returns to ready with no error', async () => {
    const { c } = await setup('cancel-permission');
    c.keyInstall();
    await settle(2000);
    expect(c.state.tag).toBe('ready');
    expect(c.error).toBeNull();
  });

  it('cancelInstall asks the backend; ready again on cancelled outcome', async () => {
    const { c } = await setup('rpm');
    c.keyInstall();
    await settle(700);
    expect(c.state.tag).toBe('awaitingPermission');
    c.cancelInstall();
    await settle(10);
    expect(c.state.tag).toBe('ready');
  });

  it('failure gives plain copy and Try Again returns to ready', async () => {
    const { c } = await setup('fail-offline');
    c.keyInstall();
    await settle();
    expect(c.state.tag).toBe('failed');
    expect(c.error).toMatchObject({
      message: 'Lumen Notes needs an internet connection to finish installing.',
      action: 'tryAgain',
      code: 'offline'
    });
    c.retry();
    expect(c.state.tag).toBe('ready');
  });

  it('incompatible package shows copy', async () => {
    const { c } = await setup('incompatible-arch');
    expect(c.state.tag).toBe('incompatible');
    expect(c.incompatible).toMatchObject({
      title: 'Lumen Notes can’t be installed here',
      body: 'This version was made for a different kind of processor.',
      action: null
    });
  });

  it('update scenario is Ready with isUpdate', async () => {
    const { c } = await setup('update');
    expect(c.state.tag).toBe('ready');
    expect(c.isUpdate).toBe(true);
  });

  it('already installed (same) offers Open, which launches then closes', async () => {
    const { c, gw } = await setup('already-installed');
    expect(c.incompatible?.action).toBe('open');
    c.openInstalled();
    await settle(50);
    expect(c.launching).toBe(true);
    expect(gw.controls.launched).toEqual(['lumen-notes']);
    await settle(OPEN_APP_CLOSE_MS);
    expect(gw.controls.windowCalls).toEqual(['close']);
    expect(c.state.tag).toBe('idle');
  });

  it('inspect failure goes to failed with the corruptFile copy', async () => {
    const gw = createMockGateways({ scenario: 'rpm', launchFile: null });
    gw.installer.openPackage = () => Promise.reject({ code: 'corruptFile', detail: 'bad magic' });
    const c = new InstallerController(gw);
    c.open('/bad');
    await settle(10);
    expect(c.state.tag).toBe('failed');
    expect(c.error).toMatchObject({ action: 'close', detail: 'bad magic' });
  });

  it('a rejected install becomes a failed outcome', async () => {
    const { c, gw } = await setup('rpm');
    gw.installer.install = () => Promise.reject(new Error('boom'));
    c.keyInstall();
    await settle(10);
    expect(c.error).toMatchObject({ code: 'unknown', detail: 'boom' });
  });

  it('a package opened during install is queued, then opened after Done', async () => {
    const { c, gw } = await setup('appimage');
    c.keyInstall();
    await settle(500);
    c.open('/second.rpm');
    expect(c.queuedCount).toBe(1);
    expect(c.appName).toBe('Lumen Notes');
    await settle();
    expect(c.state.tag).toBe('succeeded');
    c.close();
    expect(c.state.tag).toBe('inspecting');
    expect(gw.controls.windowCalls).toEqual([]);
    await settle(300);
    expect(c.state.tag).toBe('ready');
  });

  it('openApp launches the new app then closes the window', async () => {
    const { c, gw } = await setup('appimage');
    c.keyInstall();
    await settle();
    c.openApp();
    expect(c.launching).toBe(true);
    await settle(OPEN_APP_CLOSE_MS + 10);
    expect(gw.controls.launched).toEqual(['lumen-notes']);
    expect(gw.controls.windowCalls).toEqual(['close']);
  });

  it('close() closes the window when nothing is queued', async () => {
    const { c, gw } = await setup('rpm');
    c.close();
    await settle(10);
    expect(gw.controls.windowCalls).toEqual(['close']);
  });

  it('start() reacts to open-file events and cleans up', async () => {
    const gw = createMockGateways({ scenario: 'deb', launchFile: null });
    const c = new InstallerController(gw);
    const stop = c.start();
    gw.controls.emitOpenFile('/y.deb');
    await settle(300);
    expect(c.state.tag).toBe('ready');
    expect(c.preview?.format).toBe('deb');
    stop();
    gw.controls.emitOpenFile('/z.deb');
    expect(c.state.tag).toBe('ready');
  });

  it('takeLaunch opens the launch file once', async () => {
    const gw = createMockGateways({ scenario: 'rpm' });
    const c = new InstallerController(gw);
    expect(await c.takeLaunch()).toBe(true);
    await settle(300);
    expect(c.state.tag).toBe('ready');
    expect(await new InstallerController(gw).takeLaunch()).toBe(false);
  });
});
