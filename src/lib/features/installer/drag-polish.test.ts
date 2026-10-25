// @vitest-environment jsdom
// Touch drag, reduced-motion spring back, resolved-text props, failed view.
import { fireEvent, render, screen } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import DmgView from './DmgView.svelte';
import FailedView from './FailedView.svelte';
import IncompatibleView from './IncompatibleView.svelte';
import ProgressView from './ProgressView.svelte';

const rect = (left: number, top: number, w: number, h: number) =>
  ({
    left,
    top,
    right: left + w,
    bottom: top + h,
    width: w,
    height: h,
    x: left,
    y: top,
    toJSON: () => ({})
  }) as DOMRect;

function layout() {
  const icon = screen.getByRole('button', { name: /Drag into Applications/ });
  const tile = screen.getByTestId('folder-tile');
  icon.getBoundingClientRect = () => rect(0, 0, 112, 112);
  tile.getBoundingClientRect = () => rect(300, 0, 112, 112);
  return { icon, tile };
}

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  vi.useRealTimers();
  delete document.documentElement.dataset.motion;
});

describe('drag polish', () => {
  it('touch drag onto the folder installs', async () => {
    const oninstall = vi.fn();
    render(DmgView, { appName: 'A', oninstall, oncancel: vi.fn() });
    const { icon, tile } = layout();
    const p = { pointerType: 'touch', pointerId: 7, button: 0 };
    await fireEvent.pointerDown(icon, { ...p, clientX: 50, clientY: 50 });
    await fireEvent.pointerMove(icon, { ...p, clientX: 350, clientY: 50 });
    expect(tile).toHaveClass('hover');
    await fireEvent.pointerUp(icon, { ...p, clientX: 350, clientY: 50 });
    vi.advanceTimersByTime(600);
    expect(oninstall).toHaveBeenCalledOnce();
  });

  it('pointercancel springs back without installing', async () => {
    const oninstall = vi.fn();
    render(DmgView, { appName: 'A', oninstall, oncancel: vi.fn() });
    const { icon } = layout();
    await fireEvent.pointerDown(icon, { clientX: 50, clientY: 50, button: 0 });
    await fireEvent.pointerMove(icon, { clientX: 350, clientY: 50 });
    await fireEvent.pointerCancel(icon);
    vi.advanceTimersByTime(1000);
    expect(oninstall).not.toHaveBeenCalled();
    expect(icon).not.toHaveClass('dragging');
  });

  it('reduced motion: release off target snaps back at once', async () => {
    document.documentElement.dataset.motion = 'reduce';
    render(DmgView, { appName: 'A', oninstall: vi.fn(), oncancel: vi.fn() });
    const { icon } = layout();
    await fireEvent.pointerDown(icon, { clientX: 50, clientY: 50, button: 0 });
    await fireEvent.pointerMove(icon, { clientX: 150, clientY: 150 });
    await fireEvent.pointerUp(icon, { clientX: 150, clientY: 150 });
    await vi.advanceTimersByTimeAsync(50);
    expect(icon.style.transform).toContain('translate(0px, 0px)');
  });

  it('keyboard hint is linked to the icon', () => {
    render(DmgView, { appName: 'A', oninstall: vi.fn(), oncancel: vi.fn() });
    const { icon } = layout();
    const id = icon.getAttribute('aria-describedby') as string;
    expect(document.getElementById(id)).toHaveTextContent('Press Enter to install');
  });
});

describe('resolved-text props', () => {
  it('ProgressView shows controller text, live region, indeterminate bar', () => {
    render(ProgressView, { appName: 'A', percent: null, phaseText: 'Working…', etaText: '' });
    expect(screen.getByText('Working…')).toHaveAttribute('aria-live', 'polite');
    expect(screen.getByRole('progressbar')).not.toHaveAttribute('aria-valuenow');
  });

  it('IncompatibleView uses resolved title/body and offers Open on request', async () => {
    const onopen = vi.fn();
    render(IncompatibleView, {
      appName: 'A',
      title: 'T',
      body: 'B',
      canOpen: true,
      onclose: vi.fn(),
      onopen
    });
    expect(screen.getByText('T')).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Open' }));
    expect(onopen).toHaveBeenCalledOnce();
  });

  it('FailedView offers Try Again and closes with Esc', async () => {
    const onretry = vi.fn();
    const onclose = vi.fn();
    render(FailedView, { appName: 'A', message: 'Oops', canRetry: true, onretry, onclose });
    await fireEvent.click(screen.getByRole('button', { name: 'Try Again' }));
    await fireEvent.keyDown(window, { key: 'Escape' });
    expect(onretry).toHaveBeenCalledOnce();
    expect(onclose).toHaveBeenCalledOnce();
  });
});
