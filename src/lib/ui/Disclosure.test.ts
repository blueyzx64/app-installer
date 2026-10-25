// @vitest-environment jsdom
// Disclosure: collapsed by default, toggles on click.
import { fireEvent, render, screen } from '@testing-library/svelte';
import { createRawSnippet } from 'svelte';
import { describe, expect, it } from 'vitest';
import Disclosure from './Disclosure.svelte';

const body = createRawSnippet(() => ({
  render: () => '<p>Technical details</p>'
}));

describe('Disclosure', () => {
  it('starts collapsed with content hidden', () => {
    render(Disclosure, { props: { summary: 'Details', children: body } });
    const button = screen.getByRole('button', { name: 'Details' });
    expect(button).toHaveAttribute('aria-expanded', 'false');
    const region = document.getElementById(button.getAttribute('aria-controls') ?? '');
    expect(region).not.toBeNull();
    expect(region).toHaveAttribute('hidden');
  });

  it('click opens, then click closes', async () => {
    render(Disclosure, { props: { summary: 'Details', children: body } });
    const button = screen.getByRole('button', { name: 'Details' });
    const region = document.getElementById(button.getAttribute('aria-controls') ?? '');

    await fireEvent.click(button);
    expect(button).toHaveAttribute('aria-expanded', 'true');
    expect(region).not.toHaveAttribute('hidden');
    expect(screen.getByText('Technical details')).toBeVisible();

    await fireEvent.click(button);
    expect(button).toHaveAttribute('aria-expanded', 'false');
    expect(region).toHaveAttribute('hidden');
  });
});
