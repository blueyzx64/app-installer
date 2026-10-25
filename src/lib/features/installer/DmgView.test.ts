// @vitest-environment jsdom
// DmgView story + drag behaviour. Pointer events only, no HTML5 DnD.
import { fireEvent, render, screen } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import DmgView from './DmgView.svelte';

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

describe('DmgView', () => {
  it('shows the hint, labels and the password note for system installs', () => {
    render(DmgView, {
      appName: 'Lumen Notes',
      needsPassword: true,
      oninstall: vi.fn(),
      oncancel: vi.fn()
    });
    expect(screen.getByText('Drag Lumen Notes to Applications')).toBeInTheDocument();
    expect(screen.getByText('Applications')).toBeInTheDocument();
    expect(screen.getByText('Needs your password')).toBeInTheDocument();
  });

  it('says Update for an app that is already installed and hides the password note', () => {
    render(DmgView, { appName: 'A', isUpdate: true, oninstall: vi.fn(), oncancel: vi.fn() });
    expect(screen.getByText('Update')).toBeInTheDocument();
    expect(screen.queryByText('Needs your password')).toBeNull();
  });

  it('installs after the icon is dropped on the folder, once the flight is over', async () => {
    const oninstall = vi.fn();
    render(DmgView, { appName: 'A', oninstall, oncancel: vi.fn() });
    const { icon, tile } = layout();

    await fireEvent.pointerDown(icon, { clientX: 50, clientY: 50, button: 0 });
    await fireEvent.pointerMove(icon, { clientX: 350, clientY: 60 });
    expect(tile).toHaveClass('hover');
    expect(icon.style.transform).toContain('translate(300px, 10px)');

    await fireEvent.pointerUp(icon, { clientX: 350, clientY: 60 });
    expect(oninstall).not.toHaveBeenCalled();
    expect(tile).toHaveClass('gulp');
    vi.advanceTimersByTime(600);
    expect(oninstall).toHaveBeenCalledOnce();
  });

  it('springs back and does not install when released away from the folder', async () => {
    const oninstall = vi.fn();
    render(DmgView, { appName: 'A', oninstall, oncancel: vi.fn() });
    const { icon, tile } = layout();

    await fireEvent.pointerDown(icon, { clientX: 50, clientY: 50, button: 0 });
    await fireEvent.pointerMove(icon, { clientX: 150, clientY: 150 });
    expect(tile).not.toHaveClass('hover');
    await fireEvent.pointerUp(icon, { clientX: 150, clientY: 150 });
    vi.advanceTimersByTime(1000);
    expect(oninstall).not.toHaveBeenCalled();
    expect(icon).not.toHaveClass('flown');
  });

  it('Enter on the focused icon is the keyboard path to install', async () => {
    const oninstall = vi.fn();
    render(DmgView, { appName: 'A', oninstall, oncancel: vi.fn() });
    const { icon } = layout();
    icon.focus();
    await fireEvent.keyDown(icon, { key: 'Enter' });
    vi.advanceTimersByTime(600);
    expect(oninstall).toHaveBeenCalledOnce();
  });

  it('with reduced motion the wait is a short cross-fade', async () => {
    document.documentElement.dataset.motion = 'reduce';
    const oninstall = vi.fn();
    render(DmgView, { appName: 'A', oninstall, oncancel: vi.fn() });
    const { icon } = layout();
    await fireEvent.keyDown(icon, { key: ' ' });
    vi.advanceTimersByTime(130);
    expect(oninstall).toHaveBeenCalledOnce();
  });

  it('shows the permission shade while waiting and Cancel reports a cancel', async () => {
    const oncancel = vi.fn();
    render(DmgView, {
      appName: 'A',
      started: true,
      waitingForPermission: true,
      oninstall: vi.fn(),
      oncancel
    });
    expect(screen.getByText('Waiting for permission…')).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(oncancel).toHaveBeenCalledOnce();
  });
});
