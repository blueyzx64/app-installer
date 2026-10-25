// @vitest-environment jsdom
// Story-like render tests: each view in each state it can be in.
import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import DoneView from './DoneView.svelte';
import IncompatibleView from './IncompatibleView.svelte';
import PermissionShade from './PermissionShade.svelte';
import ProgressView from './ProgressView.svelte';

describe('ProgressView', () => {
  it('renders the text it is given with a determinate bar', () => {
    render(ProgressView, {
      appName: 'Lumen Notes',
      percent: 42.4,
      phaseText: 'Installing Lumen Notes…',
      etaText: 'A few seconds left'
    });
    expect(screen.getByText('Installing Lumen Notes')).toBeInTheDocument();
    expect(screen.getByText('Installing Lumen Notes…')).toHaveAttribute('aria-live', 'polite');
    expect(screen.getByText('A few seconds left')).toBeInTheDocument();
    expect(screen.getByRole('progressbar')).toHaveAttribute('aria-valuenow', '42');
  });

  it('is indeterminate when the amount is unknown', () => {
    render(ProgressView, { appName: 'A', percent: null, phaseText: 'Getting ready…' });
    expect(screen.getByRole('progressbar')).not.toHaveAttribute('aria-valuenow');
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
  it('shows the given title and body and closes', async () => {
    const onclose = vi.fn();
    render(IncompatibleView, {
      title: 'Lumen can’t be installed here',
      body: 'Body text',
      onclose
    });
    expect(screen.getByText('Lumen can’t be installed here')).toBeInTheDocument();
    expect(screen.getByText('Body text')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Open' })).toBeNull();
    await fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(onclose).toHaveBeenCalledOnce();
  });

  it('offers Open when asked', async () => {
    const onopen = vi.fn();
    render(IncompatibleView, { title: 'T', canOpen: true, onclose: vi.fn(), onopen });
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
