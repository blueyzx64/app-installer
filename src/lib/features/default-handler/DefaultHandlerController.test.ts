import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createMockGateways } from '$lib/infrastructure/mock';
import { DefaultHandlerController } from './DefaultHandlerController.svelte';

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

function setup(canShow = true) {
  const gw = createMockGateways({ scenario: 'rpm', launchFile: null });
  let show = canShow;
  const c = new DefaultHandlerController(gw, () => show);
  return { gw, c, setShow: (v: boolean) => (show = v) };
}

describe('DefaultHandlerController', () => {
  it('prompts on first launch, only while the installer allows', async () => {
    const { c, setShow } = setup(false);
    await c.check();
    expect(c.state.tag).toBe('prompting');
    expect(c.sheetVisible).toBe(false);
    setShow(true);
    expect(c.sheetVisible).toBe(true);
  });
  it('stays hidden when already default or asked', async () => {
    const a = setup();
    await a.gw.defaults.dismissPrompt();
    await a.c.check();
    expect(a.c.state.tag).toBe('hidden');
    expect(a.c.sheetVisible).toBe(false);
  });
  it('Make Default shows the toast for 4 s', async () => {
    const { c, gw } = setup();
    await c.check();
    await c.makeDefault();
    expect(gw.controls.defaults.isDefault).toBe(true);
    expect(c.toastVisible).toBe(true);
    expect(c.sheetVisible).toBe(false);
    await vi.advanceTimersByTimeAsync(3999);
    expect(c.toastVisible).toBe(true);
    await vi.advanceTimersByTimeAsync(2);
    expect(c.toastVisible).toBe(false);
    expect(c.state.tag).toBe('hidden');
  });
  it('Not Now hides and remembers', async () => {
    const { c, gw } = setup();
    await c.check();
    await c.notNow();
    expect(c.state.tag).toBe('hidden');
    expect(gw.controls.defaults.askedBefore).toBe(true);
  });
  it('apply failure is silent', async () => {
    const { c, gw } = setup();
    gw.defaults.makeDefault = () => Promise.reject(new Error('x'));
    await c.check();
    await c.makeDefault();
    expect(c.state.tag).toBe('hidden');
    expect(c.toastVisible).toBe(false);
  });
  it('apply failure is remembered as asked (no nagging every launch)', async () => {
    const { c, gw } = setup();
    gw.defaults.makeDefault = () => Promise.reject(new Error('x'));
    await c.check();
    await c.makeDefault();
    expect(gw.controls.defaults.askedBefore).toBe(true);
    expect(c.state.tag).toBe('hidden');
  });
  it('status failure hides', async () => {
    const { c, gw } = setup();
    gw.defaults.status = () => Promise.reject(new Error('x'));
    await c.check();
    expect(c.state.tag).toBe('hidden');
  });
});
