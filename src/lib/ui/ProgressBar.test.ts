// @vitest-environment jsdom
import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import ProgressBar from './ProgressBar.svelte';

describe('ProgressBar', () => {
  it('exposes a progressbar role with the label', () => {
    render(ProgressBar, { value: 40, label: 'Copying' });
    const bar = screen.getByRole('progressbar', { name: 'Copying' });
    expect(bar).toHaveAttribute('aria-valuemin', '0');
    expect(bar).toHaveAttribute('aria-valuemax', '100');
  });

  it('rounds aria-valuenow to a whole number', () => {
    render(ProgressBar, { value: 42.6, label: 'Copying' });
    expect(screen.getByRole('progressbar')).toHaveAttribute('aria-valuenow', '43');
  });

  it('clamps values below 0 to 0', () => {
    render(ProgressBar, { value: -5, label: 'Copying' });
    expect(screen.getByRole('progressbar')).toHaveAttribute('aria-valuenow', '0');
  });

  it('clamps values above 100 to 100', () => {
    render(ProgressBar, { value: 150, label: 'Copying' });
    expect(screen.getByRole('progressbar')).toHaveAttribute('aria-valuenow', '100');
  });

  it('is indeterminate when value is null', () => {
    const { container } = render(ProgressBar, { value: null, label: 'Working' });
    const bar = screen.getByRole('progressbar');
    expect(bar).not.toHaveAttribute('aria-valuenow');
    expect(container.querySelector('.indeterminate')).not.toBeNull();
  });

  it('shows no indeterminate segment when a value is set', () => {
    const { container } = render(ProgressBar, { value: 10, label: 'Working' });
    expect(container.querySelector('.indeterminate')).toBeNull();
  });
});
