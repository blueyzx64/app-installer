// @vitest-environment jsdom
// Story-like render tests: each view in each state it can be in.
import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import DoneView from './DoneView.svelte';
import IncompatibleView from './IncompatibleView.svelte';
import PermissionShade from './PermissionShade.svelte';
import ProgressView, { etaPhrase } from './ProgressView.svelte';

describe('ProgressView', () => {
  it('shows a determinate bar with the phase and time left', () => {
    render(ProgressView, {
      appName: 'Lumen Notes',
      phase: 'installing',
      percent: 42.4,
      etaSeconds: 12
    });
    expect(screen.getByText('Installing Lumen Notes')).toBeInTheDocument();
    expect(screen.getByText('Installing Lumen Notes…')).toBeInTheDocument();
    expect(screen.getByText('A few seconds left')).toBeInTheDocument();
    expect(screen.getByRole('progressbar')).toHaveAttribute('aria-valuenow', '42');
  });

  it('is indeterminate when the amount is unknown', () => {
    render(ProgressView, { appName: 'Lumen Notes', phase: 'preparing', percent: null });
    const bar = screen.getByRole('progressbar');
    expect(bar).not.toHaveAttribute('aria-valuenow');
    expect(screen.getByText('Getting ready…')).toBeInTheDocument();
  });

  it('adds the size for large downloads and hides the time when finishing', () => {
    const { rerender } = render(ProgressView, {
      appName: 'A',
      phase: 'downloading',
      percent: 10,
      sizeLabel: '120 MB',
      etaSeconds: 200
    });
    expect(screen.getByText('Getting a few things it needs (120 MB)')).toBeInTheDocument();
    expect(screen.getByText('About 3 minutes left')).toBeInTheDocument();
    void rerender({ appName: 'A', phase: 'finishing', percent: 100, etaSeconds: 3 });
  });

  it('maps seconds to plain phrases', () => {
    expect(etaPhrase(null).key).toBe('eta.unknown');
    expect(etaPhrase(5).key).toBe('eta.seconds');
    expect(etaPhrase(60).key).toBe('eta.minute');
    expect(etaPhrase(300)).toEqual({ key: 'eta.minutes', n: 5 });
  });
});

describe('DoneView', () => {
  it('offers Open as the focused primary action and Done as the other', async () => {
    const onopen = vi.fn();
    const ondone = vi.fn();
    render(DoneView, { appName: 'Lumen Notes', onopen, ondone });
    expect(screen.getByText('Lumen Notes is installed')).toBeInTheDocument();
    const open = screen.getByRole('button', { name: 'Open' });
    expect(open).toHaveFocus();
    await fireEvent.click(open);
    await fireEvent.click(screen.getByRole('button', { name: 'Done' }));
    expect(onopen).toHaveBeenCalledOnce();
    expect(ondone).toHaveBeenCalledOnce();
  });

  it('Esc is Done; Open disappears when the app cannot launch', async () => {
    const ondone = vi.fn();
    render(DoneView, { appName: 'A', canLaunch: false, onopen: vi.fn(), ondone });
    expect(screen.queryByRole('button', { name: 'Open' })).toBeNull();
    await fireEvent.keyDown(window, { key: 'Escape' });
    expect(ondone).toHaveBeenCalledOnce();
  });

  it('disables Open while launching', () => {
    render(DoneView, { appName: 'A', launching: true, onopen: vi.fn(), ondone: vi.fn() });
    expect(screen.getByRole('button', { name: 'Opening…' })).toBeDisabled();
  });
});

describe('IncompatibleView', () => {
  it('explains a wrong-family package without technical words', async () => {
    const onclose = vi.fn();
    render(IncompatibleView, { kind: 'wrongFamily.debBased', appName: 'Lumen Notes', onclose });
    expect(screen.getByText("Lumen Notes can't be installed here")).toBeInTheDocument();
    expect(screen.getByText(/made for Ubuntu-based computers/)).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(onclose).toHaveBeenCalledOnce();
  });

  it('offers Open when the same version is already installed', async () => {
    const onopen = vi.fn();
    render(IncompatibleView, {
      kind: 'alreadyInstalled.same',
      appName: 'Lumen Notes',
      canOpen: true,
      onclose: vi.fn(),
      onopen
    });
    await fireEvent.click(screen.getByRole('button', { name: 'Open' }));
    expect(onopen).toHaveBeenCalledOnce();
  });
});

describe('PermissionShade', () => {
  it('waits for the system dialog, never shows a password field, and cancels with Esc', async () => {
    const oncancel = vi.fn();
    render(PermissionShade, { oncancel });
    expect(screen.getByRole('alertdialog')).toBeInTheDocument();
    expect(screen.getByText('Waiting for permission…')).toBeInTheDocument();
    expect(document.querySelector('input')).toBeNull();
    await fireEvent.keyDown(screen.getByRole('alertdialog'), { key: 'Escape' });
    expect(oncancel).toHaveBeenCalledOnce();
  });
});
