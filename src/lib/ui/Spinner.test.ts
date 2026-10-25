// @vitest-environment jsdom
import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import Spinner from './Spinner.svelte';

describe('Spinner', () => {
  it('is hidden from assistive tech when no label is given', () => {
    const { container } = render(Spinner);
    const spinner = container.querySelector('.spinner');
    expect(spinner).toHaveAttribute('aria-hidden', 'true');
    expect(spinner).not.toHaveAttribute('role');
  });

  it('exposes role=status and aria-label when a label is given', () => {
    render(Spinner, { label: 'Saving' });
    const status = screen.getByRole('status', { name: 'Saving' });
    expect(status).toHaveAttribute('aria-label', 'Saving');
    expect(status).not.toHaveAttribute('aria-hidden');
  });
});
