// @vitest-environment jsdom
import { fireEvent, render, screen } from '@testing-library/svelte';
import { createRawSnippet } from 'svelte';
import { describe, expect, it } from 'vitest';
import AppIcon from './AppIcon.svelte';

describe('AppIcon', () => {
  it('renders an img with the given src', () => {
    const { container } = render(AppIcon, { src: 'data:image/png;base64,AAAA' });
    const img = container.querySelector('img');
    expect(img).not.toBeNull();
    expect(img).toHaveAttribute('src', 'data:image/png;base64,AAAA');
  });

  it('shows the generic glyph and no img when src is missing', () => {
    const { container } = render(AppIcon, {});
    expect(container.querySelector('img')).toBeNull();
    expect(container.querySelector('svg')).not.toBeNull();
  });

  it('falls back to the glyph when the image fails to load', async () => {
    const { container } = render(AppIcon, { src: 'data:image/png;base64,BROKEN' });
    const img = container.querySelector('img') as HTMLImageElement;
    await fireEvent.error(img);
    expect(container.querySelector('img')).toBeNull();
    expect(container.querySelector('svg')).not.toBeNull();
  });

  it('exposes role img and aria-label when labelled', () => {
    render(AppIcon, { label: 'Lumen Notes' });
    const icon = screen.getByRole('img', { name: 'Lumen Notes' });
    expect(icon).not.toHaveAttribute('aria-hidden');
  });

  it('is aria-hidden when no label is given', () => {
    const { container } = render(AppIcon, {});
    const root = container.querySelector('.icon');
    expect(root).toHaveAttribute('aria-hidden', 'true');
    expect(root).not.toHaveAttribute('role');
    expect(screen.queryByRole('img')).toBeNull();
  });

  it('renders the badge snippet', () => {
    const badge = createRawSnippet(() => ({
      render: () => '<span class="test-badge">lock</span>'
    }));
    const { container } = render(AppIcon, { badge });
    expect(container.querySelector('.badge .test-badge')).not.toBeNull();
  });
});
