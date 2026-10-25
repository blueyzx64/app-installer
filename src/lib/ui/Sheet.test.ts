// @vitest-environment jsdom
import { createRawSnippet } from 'svelte';
import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import Sheet from './Sheet.svelte';

const actions = createRawSnippet(() => ({
  render: () => '<div><button class="primary">Continue</button></div>'
}));

function mountSheet(oncancel = vi.fn()) {
  render(Sheet, {
    props: { title: 'Install Demo', body: 'Drag the icon to Applications.', actions, oncancel }
  });
  return { oncancel, dialog: screen.getByRole('dialog') };
}

describe('Sheet', () => {
  it('is a modal dialog named by its title', () => {
    const { dialog } = mountSheet();
    expect(dialog).toHaveAttribute('aria-modal', 'true');
    expect(dialog).toHaveAccessibleName('Install Demo');
  });

  it('renders title and body', () => {
    mountSheet();
    expect(screen.getByRole('heading', { name: 'Install Demo' })).toBeInTheDocument();
    expect(screen.getByText('Drag the icon to Applications.')).toBeInTheDocument();
  });

  it('calls oncancel on Escape', () => {
    const { oncancel, dialog } = mountSheet();
    fireEvent.keyDown(dialog, { key: 'Escape' });
    expect(oncancel).toHaveBeenCalledTimes(1);
  });
});
