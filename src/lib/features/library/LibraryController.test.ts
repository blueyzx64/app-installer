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
  it('trash confirm copy differs for user and system apps', async () => {
    const { c } = await setup('rpm');
    c.requestTrash('pixel-draw');
    expect(c.confirm).toMatchObject({
      title: 'Move “Pixel Draw” to the Trash?',
      confirmLabel: 'Move to Trash',
      system: false
    });
    c.cancelTrash();
    expect(c.state.tag).toBe('listing');
    c.requestTrash('orbit-player');
    expect(c.confirm).toMatchObject({ title: 'Remove “Orbit Player”?', system: true });
  });
  it('confirm removes the app after the fake uninstall', async () => {
    const { c } = await setup('rpm');
    c.requestTrash('pixel-draw');
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
    c.requestTrash('orbit-player');
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
    c.requestTrash('orbit-player');
    const p = c.confirmTrash();
    await vi.advanceTimersByTimeAsync(5000);
    await p;
    expect(c.notice).toBeNull();
    expect(c.apps.map((a) => a.id)).toContain('orbit-player');
  });
  it('failure shows a plain toast that hides itself', async () => {
    const { c } = await setup('fail-offline');
    c.requestTrash('orbit-player');
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
    c.requestTrash('pixel-draw');
    await c.confirmTrash();
    expect(c.notice).toBe('You don’t have permission to remove Pixel Draw.');
    c.dismissNotice();
    expect(c.notice).toBeNull();
  });
  it('launch calls the gateway and swallows errors', async () => {
    const { c, gw } = await setup('rpm');
    await c.launch('pixel-draw');
    await c.launch('nope');
    expect(gw.controls.launched).toEqual(['pixel-draw']);
  });
});
