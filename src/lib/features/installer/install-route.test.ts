// @vitest-environment jsdom
import { render } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Controllers } from '$lib/app/context';
import { createMockGateways, type Scenario } from '$lib/infrastructure/mock';
import Host from './review-r7-host.svelte';

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());
const settle = (ms: number) => vi.advanceTimersByTimeAsync(ms);
const escape = () => window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));

async function mount(scenario: Scenario) {
  const gw = createMockGateways({ scenario, launchFile: null });
  let ctl!: Controllers;
  render(Host, { gateways: gw, onready: (c: unknown) => (ctl = c as Controllers) });
  return { gw, ctl };
}

describe('install route keyboard', () => {
  it('F-21: the app icon has focus when the ready screen appears', async () => {
    const { ctl } = await mount('rpm');
    ctl.installer.open('/x.rpm');
    await settle(300);
    expect(ctl.installer.state.tag).toBe('ready');
    const icon = document.querySelector('button.icon');
    expect(icon).not.toBeNull();
    expect(document.activeElement).toBe(icon);
  });

  it('Escape is ignored while an install is running', async () => {
    const { gw, ctl } = await mount('rpm');
    ctl.installer.open('/x.rpm');
    await settle(300);
    ctl.installer.keyInstall();
    await settle(100);
    escape();
    expect(ctl.installer.busy).toBe(true);
    expect(gw.controls.windowCalls).not.toContain('close');
  });

  it('Escape on the error screen acts like its Close button', async () => {
    const { gw, ctl } = await mount('rpm');
    ctl.installer.open('/x.rpm');
    await settle(300);
    vi.spyOn(gw.installer, 'install').mockResolvedValue({
      kind: 'failed',
      code: 'unknown',
      detail: null
    });
    ctl.installer.keyInstall();
    await settle(50);
    expect(ctl.installer.state.tag).toBe('failed');
    (document.activeElement as HTMLElement | null)?.blur();
    escape();
    expect(ctl.installer.state.tag).toBe('idle');
    expect(gw.controls.windowCalls).toContain('close');
  });

  it('Escape on the error screen opens the next waiting package instead of closing', async () => {
    const { gw, ctl } = await mount('rpm');
    ctl.installer.open('/x.rpm');
    await settle(300);
    vi.spyOn(gw.installer, 'install').mockImplementation(async () => {
      ctl.installer.open('/next.rpm'); // arrives mid-install: queued
      return { kind: 'failed', code: 'unknown', detail: null };
    });
    ctl.installer.keyInstall();
    await settle(50);
    expect(ctl.installer.state.tag).toBe('failed');
    (document.activeElement as HTMLElement | null)?.blur();
    escape();
    expect(ctl.installer.state.tag).toBe('inspecting');
    expect(gw.controls.windowCalls).not.toContain('close');
  });
});
