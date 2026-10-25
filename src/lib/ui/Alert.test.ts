// @vitest-environment jsdom
import { createRawSnippet } from 'svelte';
import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import Alert from './Alert.svelte';

const actions = createRawSnippet(() => ({
  render: () =>
    '<div><button class="primary">Keep</button><button class="secondary">Cancel</button></div>'
}));

function mountAlert(oncancel = vi.fn()) {
  render(Alert, {
    props: { title: 'Move to Applications?', message: 'Pick a folder.', actions, oncancel }
  });
  return { oncancel, dialog: screen.getByRole('alertdialog') };
}

describe('Alert', () => {
  it('is an alertdialog named by its title and described by its message', () => {
    const { dialog } = mountAlert();
    expect(dialog).toHaveAttribute('aria-modal', 'true');
    expect(dialog).toHaveAccessibleName('Move to Applications?');
    expect(dialog).toHaveAccessibleDescription('Pick a folder.');
  });

  it('focuses the primary button first', () => {
    mountAlert();
    expect(screen.getByRole('button', { name: 'Keep' })).toHaveFocus();
  });

  it('calls oncancel on Escape', () => {
    const { oncancel, dialog } = mountAlert();
    fireEvent.keyDown(dialog, { key: 'Escape' });
    expect(oncancel).toHaveBeenCalledTimes(1);
  });

  it('wraps Tab from last to first', () => {
    const { dialog } = mountAlert();
    const keep = screen.getByRole('button', { name: 'Keep' });
    const cancel = screen.getByRole('button', { name: 'Cancel' });
    cancel.focus();
    fireEvent.keyDown(dialog, { key: 'Tab' });
    expect(keep).toHaveFocus();
  });

  it('wraps Shift+Tab from first to last', () => {
    const { dialog } = mountAlert();
    const keep = screen.getByRole('button', { name: 'Keep' });
    const cancel = screen.getByRole('button', { name: 'Cancel' });
    keep.focus();
    fireEvent.keyDown(dialog, { key: 'Tab', shiftKey: true });
    expect(cancel).toHaveFocus();
  });
});
