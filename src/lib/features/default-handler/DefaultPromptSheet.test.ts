// @vitest-environment jsdom
import { fireEvent, render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createMockGateways } from '$lib/infrastructure/mock';
import { DefaultHandlerController } from './DefaultHandlerController.svelte';
import DefaultPromptSheet from './DefaultPromptSheet.svelte';

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

async function mount(canShow = true) {
  const gw = createMockGateways({ scenario: 'rpm', launchFile: null });
  const controller = new DefaultHandlerController(gw, () => canShow);
  render(DefaultPromptSheet, { props: { controller } });
  await vi.advanceTimersByTimeAsync(0);
  await tick();
  return { gw, controller };
}

describe('DefaultPromptSheet', () => {
  it('shows the sheet with both buttons on first launch', async () => {
    await mount();
    expect(screen.getByRole('dialog')).toHaveAccessibleName('Open app files with Installer?');
    expect(screen.getByRole('button', { name: 'Make Default' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Not Now' })).toBeInTheDocument();
  });

  it('stays hidden when the installer is busy', async () => {
    await mount(false);
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('Not Now closes and remembers', async () => {
    const { gw } = await mount();
    await fireEvent.click(screen.getByRole('button', { name: 'Not Now' }));
    await vi.advanceTimersByTimeAsync(0);
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(gw.controls.defaults.askedBefore).toBe(true);
  });

  it('Esc counts as Not Now', async () => {
    const { gw } = await mount();
    await fireEvent.keyDown(screen.getByRole('dialog'), { key: 'Escape' });
    await vi.advanceTimersByTimeAsync(0);
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(gw.controls.defaults.askedBefore).toBe(true);
  });

  it('Make Default shows the toast, then it goes away after 4 s', async () => {
    const { gw } = await mount();
    await fireEvent.click(screen.getByRole('button', { name: 'Make Default' }));
    await vi.advanceTimersByTimeAsync(0);
    expect(gw.controls.defaults.isDefault).toBe(true);
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(screen.getByRole('status')).toHaveTextContent('Installer will now open app files');
    await vi.advanceTimersByTimeAsync(4100);
    await tick();
    expect(screen.queryByRole('status')).toBeNull();
  });

  it('renders nothing when already asked', async () => {
    const gw = createMockGateways({ scenario: 'rpm', launchFile: null });
    await gw.defaults.dismissPrompt();
    const controller = new DefaultHandlerController(gw, () => true);
    render(DefaultPromptSheet, { props: { controller } });
    await vi.advanceTimersByTimeAsync(0);
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(screen.queryByRole('status')).toBeNull();
  });
});
