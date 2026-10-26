import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createMockGateways, type Scenario } from '$lib/infrastructure/mock';
import { LibraryController, NOTICE_MS } from './LibraryController.svelte';

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

async function setup(scenario: Scenario) {
  const gw = createMockGateways({ scenario, launchFile: null });
  const c = new LibraryController(gw);
  await c.load();
  return { gw, c };
}

describe('LibraryController', () => {
  it('loads apps', async () => {
    const { c } = await setup('rpm');
    expect(c.state.tag).toBe('listing');
    expect(c.apps.length).toBe(3);
  });
  it('empty library', async () => {
    const { c } = await setup('empty-library');
    expect(c.state.tag).toBe('empty');
  });
  it('load failure shows empty without a notice', async () => {
    const gw = createMockGateways({ scenario: 'rpm' });
    gw.library.listInstalled = () => Promise.reject({ code: 'backendUnavailable', detail: null });
    const c = new LibraryController(gw);
    await c.load();
    expect(c.state.tag).toBe('empty');
    expect(c.notice).toBeNull();
  });
  it('a slow earlier load never overwrites a newer one', async () => {
    const gw = createMockGateways({ scenario: 'rpm', launchFile: null });
    const stale = await gw.library.listInstalled(); // three seeded apps
    let releaseOld!: (apps: typeof stale) => void;
    gw.library.listInstalled = vi
      .fn()
      .mockImplementationOnce(() => new Promise<typeof stale>((r) => (releaseOld = r)))
      .mockImplementationOnce(() => Promise.resolve([]));
    const c = new LibraryController(gw);
    const first = c.load();
    await c.load();
    expect(c.state.tag).toBe('empty');
    releaseOld(stale);
    await first;
    expect(c.state.tag).toBe('empty');
  });
  it('trash confirm copy differs for user and system apps', async () => {
    const { c } = await setup('rpm');
    await c.requestTrash('pixel-draw');
    expect(c.confirm).toMatchObject({
      title: 'Move “Pixel Draw” to the Trash?',
      confirmLabel: 'Move to Trash',
      system: false
    });
    c.cancelTrash();
    expect(c.state.tag).toBe('listing');
    await c.requestTrash('orbit-player');
    expect(c.confirm).toMatchObject({ title: 'Remove “Orbit Player”?', system: true });
  });
  it('system app alert lists the extra items once the plan arrives (plural rules)', async () => {
    const { c, gw } = await setup('rpm');
    const p = c.requestTrash('orbit-player');
    expect(c.confirm?.ready).toBe(false);
    await p;
    expect(c.confirm?.ready).toBe(true);
    expect(c.confirm?.body).toBe(
      'You’ll be asked for your password. This also removes 2 other items that only Orbit Player used.'
    );
    c.cancelTrash();
    gw.library.uninstallPlan = async () => ({ alsoRemoves: 1, needsPermission: true });
    await c.requestTrash('orbit-player');
    expect(c.confirm?.body).toContain('1 other item that only Orbit Player used');
    c.cancelTrash();
    gw.library.uninstallPlan = async () => ({ alsoRemoves: 0, needsPermission: true });
    await c.requestTrash('orbit-player');
    expect(c.confirm?.body).toBe('You’ll be asked for your password.');
  });
  it('user app alert never mentions extras', async () => {
    const { c } = await setup('rpm');
    await c.requestTrash('pixel-draw');
    expect(c.confirm?.body).toBe('You can restore it from the Trash later.');
  });
  it('a failed plan still lets the person confirm', async () => {
    const { c, gw } = await setup('rpm');
    gw.library.uninstallPlan = () => Promise.reject({ code: 'unknown', detail: null });
    await c.requestTrash('orbit-player');
    expect(c.confirm?.ready).toBe(true);
    expect(c.confirm?.body).toBe(
      'You’ll be asked for your password. This may also remove other items that only Orbit Player used.'
    );
  });
  it('confirm before the plan arrives does nothing', async () => {
    const { c, gw } = await setup('rpm');
    gw.library.uninstallPlan = () => new Promise(() => {});
    const spy = vi.spyOn(gw.library, 'uninstall');
    void c.requestTrash('orbit-player');
    await c.confirmTrash();
    expect(spy).not.toHaveBeenCalled();
    expect(c.state.tag).toBe('confirming');
  });
  it('cancel before the plan arrives drops the late plan', async () => {
    const { c } = await setup('rpm');
    const p = c.requestTrash('orbit-player');
    c.cancelTrash();
    await p;
    expect(c.state.tag).toBe('listing');
  });
  it('unknown app id does nothing', async () => {
    const { c } = await setup('rpm');
    await c.requestTrash('nope');
    expect(c.state.tag).toBe('listing');
  });
  it('confirm removes the app after the fake uninstall', async () => {
    const { c } = await setup('rpm');
    await c.requestTrash('pixel-draw');
    const p = c.confirmTrash();
    expect(c.state.tag).toBe('removing');
    expect(c.removingId).toBe('pixel-draw');
    expect(c.removalText).toBe('Removing Pixel Draw…');
    await vi.advanceTimersByTimeAsync(5000);
    await p;
    expect(c.state.tag).toBe('listing');
    expect(c.apps.map((a) => a.id)).not.toContain('pixel-draw');
  });
  it('system removal shows the permission line first', async () => {
    const { c } = await setup('rpm');
    await c.requestTrash('orbit-player');
    const p = c.confirmTrash();
    await vi.advanceTimersByTimeAsync(10);
    expect(c.removalText).toBe('Waiting for permission…');
    await vi.advanceTimersByTimeAsync(1200);
    expect(c.removalText).toBe('Removing Orbit Player…');
    await vi.advanceTimersByTimeAsync(5000);
    await p;
  });
  it('cancelled password prompt is silent and keeps the app', async () => {
    const { c } = await setup('cancel-permission');
    await c.requestTrash('orbit-player');
    const p = c.confirmTrash();
    await vi.advanceTimersByTimeAsync(5000);
    await p;
    expect(c.notice).toBeNull();
    expect(c.apps.map((a) => a.id)).toContain('orbit-player');
  });
  it('failure shows a plain toast that hides itself', async () => {
    const { c } = await setup('fail-offline');
    await c.requestTrash('orbit-player');
    const p = c.confirmTrash();
    await vi.advanceTimersByTimeAsync(5000);
    await p;
    expect(c.notice).toBe('Something went wrong while removing Orbit Player.');
    await vi.advanceTimersByTimeAsync(NOTICE_MS + 10);
    expect(c.notice).toBeNull();
  });
  it('a rejected uninstall also becomes a toast', async () => {
    const { c, gw } = await setup('rpm');
    gw.library.uninstall = () => Promise.reject({ code: 'permissionDenied', detail: null });
    await c.requestTrash('pixel-draw');
    await c.confirmTrash();
    expect(c.notice).toBe('You don’t have permission to remove Pixel Draw.');
    c.dismissNotice();
    expect(c.notice).toBeNull();
  });
  it('a list that lands during removal is applied after, without the removed app', async () => {
    const { c, gw } = await setup('rpm');
    await c.requestTrash('pixel-draw');
    const p = c.confirmTrash();
    const stale = [...c.apps];
    const real = gw.library.listInstalled;
    gw.library.listInstalled = async () => stale.filter((a) => a.id !== 'tidy-tasks');
    await c.load(); // removal still running: kept as pending
    expect(c.state.tag).toBe('removing');
    gw.library.listInstalled = real;
    await vi.advanceTimersByTimeAsync(5000);
    await p;
    expect(c.apps.map((a) => a.id)).toEqual(['orbit-player']);
  });
  it('launch calls the gateway', async () => {
    const { c, gw } = await setup('rpm');
    await c.launch('pixel-draw');
    expect(gw.controls.launched).toEqual(['pixel-draw']);
    expect(c.notice).toBeNull();
  });
  it('a failed open shows a plain toast that hides itself', async () => {
    const { c, gw } = await setup('rpm');
    gw.library.launchApp = () => Promise.reject({ code: 'unknown', detail: 'exec failed' });
    await c.launch('pixel-draw');
    expect(c.notice).toBe('Pixel Draw couldn’t be opened.');
    await vi.advanceTimersByTimeAsync(NOTICE_MS + 10);
    expect(c.notice).toBeNull();
    await c.launch('pixel-draw');
    c.dismissNotice();
    expect(c.notice).toBeNull();
  });
});
