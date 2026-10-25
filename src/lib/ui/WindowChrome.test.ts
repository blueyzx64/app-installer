// @vitest-environment jsdom
// Window controls: which buttons, which side, actions, drag fallback, unfocused look.
import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import type { Appearance } from '$lib/core/domain/types';
type Controls = Appearance['controls'];
import type { WindowGateway } from '$lib/core/ports';
import WindowChrome from './WindowChrome.svelte';
import WindowControls from './WindowControls.svelte';

const GNOME: Controls = { side: 'right', buttons: ['close'] };
const MAC: Controls = { side: 'left', buttons: ['close', 'minimize', 'maximize'] };

function gw(): WindowGateway & { calls: string[] } {
  const calls: string[] = [];
  return {
    calls,
    close: async () => void calls.push('close'),
    minimize: async () => void calls.push('minimize'),
    startDragging: async () => void calls.push('startDragging')
  };
}

describe('WindowControls', () => {
  it('appmenu:close shows one close button', () => {
    render(WindowControls, { props: { controls: GNOME } });
    expect(screen.getAllByRole('button')).toHaveLength(1);
    expect(screen.getByRole('button', { name: 'Close' })).toBeInTheDocument();
    expect(screen.getByRole('group')).toHaveAttribute('data-side', 'right');
  });

  it('close,minimize,maximize: shows three circles in order with zoom disabled', () => {
    const { container } = render(WindowControls, { props: { controls: MAC } });
    const dots = [...container.querySelectorAll('button')];
    expect(dots.map((d) => ['close', 'min', 'zoom'].find((k) => d.classList.contains(k)))).toEqual([
      'close',
      'min',
      'zoom'
    ]);
    expect(dots[2]).toBeDisabled();
    expect(screen.getByRole('group')).toHaveAttribute('data-side', 'left');
  });

  it('omits maximize when the setting omits it', () => {
    const { container } = render(WindowControls, {
      props: { controls: { side: 'right', buttons: ['minimize', 'close'] } }
    });
    expect(container.querySelector('.zoom')).toBeNull();
    expect(container.querySelectorAll('button')).toHaveLength(2);
  });

  it('calls the handlers', async () => {
    const onclose = vi.fn();
    const onminimize = vi.fn();
    render(WindowControls, { props: { controls: MAC, onclose, onminimize } });
    await fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Minimize' }));
    expect(onclose).toHaveBeenCalledOnce();
    expect(onminimize).toHaveBeenCalledOnce();
  });

  it('always renders close, even if the layout lacks it', () => {
    render(WindowControls, {
      props: { controls: { side: 'right', buttons: ['minimize', 'maximize'] } }
    });
    expect(screen.getByRole('button', { name: 'Close' })).toBeInTheDocument();
  });

  it('marks unfocused windows', () => {
    const { container } = render(WindowControls, { props: { controls: GNOME, focused: false } });
    expect(container.querySelector('.controls')).toHaveClass('unfocused');
  });
});

describe('WindowChrome', () => {
  it('has a drag region, the title, and the body', () => {
    const { container } = render(WindowChrome, {
      props: { title: 'Installing Lumen', window: gw(), controls: MAC, children: textSnippet() }
    });
    expect(container.querySelector('[data-tauri-drag-region]')).not.toBeNull();
    expect(screen.getByRole('heading', { name: 'Installing Lumen' })).toBeInTheDocument();
    expect(container.querySelector('[data-side="left"]')).not.toBeNull();
  });

  it('close and minimize go to the window gateway', async () => {
    const w = gw();
    render(WindowChrome, {
      props: { title: 'x', window: w, controls: MAC, children: textSnippet() }
    });
    await fireEvent.click(screen.getByRole('button', { name: 'Minimize' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(w.calls).toEqual(['minimize', 'close']);
  });

  it('pressing the empty bar starts a drag; pressing a button does not', async () => {
    const w = gw();
    const { container } = render(WindowChrome, {
      props: { title: 'x', window: w, children: textSnippet() }
    });
    await fireEvent.pointerDown(screen.getByRole('button', { name: 'Close' }), { button: 0 });
    expect(w.calls).toEqual([]);
    await fireEvent.pointerDown(container.querySelector('header')!, { button: 2 });
    expect(w.calls).toEqual([]);
    await fireEvent.pointerDown(container.querySelector('header')!, { button: 0 });
    expect(w.calls).toEqual(['startDragging']);
  });

  it('greys the controls on blur and restores on focus', async () => {
    const { container } = render(WindowChrome, {
      props: { title: 'x', window: gw(), children: textSnippet() }
    });
    await fireEvent(window, new Event('blur'));
    expect(container.querySelector('.controls')).toHaveClass('unfocused');
    await fireEvent(window, new Event('focus'));
    expect(container.querySelector('.controls')).not.toHaveClass('unfocused');
  });
});

import { createRawSnippet } from 'svelte';
function textSnippet() {
  return createRawSnippet(() => ({ render: () => '<p>body</p>' }));
}
