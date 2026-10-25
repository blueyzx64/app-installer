// @vitest-environment jsdom
// Toast: status role, text, auto-dismiss timing, and cleanup on unmount.
import { render, screen } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import Toast from './Toast.svelte';

describe('Toast', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('renders a polite status region with the title and body', () => {
    render(Toast, { props: { title: 'Installed', body: 'Ready to open', ondismiss: () => {} } });

    const status = screen.getByRole('status');
    expect(status).toHaveAttribute('aria-live', 'polite');
    expect(screen.getByText('Installed')).toBeInTheDocument();
    expect(screen.getByText('Ready to open')).toBeInTheDocument();
  });

  it('dismisses after 4000 ms, not before', () => {
    const ondismiss = vi.fn();
    render(Toast, { props: { title: 'Saved', ondismiss } });

    vi.advanceTimersByTime(3999);
    expect(ondismiss).not.toHaveBeenCalled();

    vi.advanceTimersByTime(1);
    expect(ondismiss).toHaveBeenCalledTimes(1);
  });

  it('does not dismiss after unmount', () => {
    const ondismiss = vi.fn();
    const { unmount } = render(Toast, { props: { title: 'Saved', ondismiss } });

    unmount();
    vi.advanceTimersByTime(10_000);
    expect(ondismiss).not.toHaveBeenCalled();
  });
});
