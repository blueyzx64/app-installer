// @vitest-environment jsdom
import { fireEvent, render, screen } from '@testing-library/svelte';
import { createRawSnippet } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import Button from './Button.svelte';

const label = (text: string) => createRawSnippet(() => ({ render: () => `<span>${text}</span>` }));

describe('Button', () => {
  it('renders its label', () => {
    render(Button, { children: label('Save') });
    expect(screen.getByRole('button', { name: 'Save' })).toBeInTheDocument();
  });

  it('defaults to type=button', () => {
    render(Button, { children: label('Save') });
    expect(screen.getByRole('button')).toHaveAttribute('type', 'button');
  });

  it('fires onclick when clicked', async () => {
    const onclick = vi.fn();
    render(Button, { children: label('Save'), onclick });
    await fireEvent.click(screen.getByRole('button'));
    expect(onclick).toHaveBeenCalledTimes(1);
  });

  // jsdom still dispatches click events to disabled buttons, so the block is
  // asserted through the native disabled attribute (browsers skip the click).
  it('blocks clicks when disabled', () => {
    render(Button, { children: label('Save'), disabled: true });
    expect(screen.getByRole('button')).toBeDisabled();
  });

  it('when busy: disabled, aria-busy, shows spinner', () => {
    const { container } = render(Button, { children: label('Save'), busy: true });
    const button = screen.getByRole('button', { name: 'Save' });
    expect(button).toBeDisabled();
    expect(button).toHaveAttribute('aria-busy', 'true');
    expect(container.querySelector('.spinner')).not.toBeNull();
  });

  it.each(['primary', 'secondary', 'destructive'] as const)(
    'applies the %s variant class',
    (variant) => {
      render(Button, { children: label('Save'), variant });
      expect(screen.getByRole('button')).toHaveClass('btn', variant);
    }
  );
});
