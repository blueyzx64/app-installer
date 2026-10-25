// @vitest-environment jsdom
import { describe, expect, it } from 'vitest';
import type { Appearance } from '$lib/core/domain/types';
import type { AppearanceGateway } from '$lib/core/ports';
import { AppearanceStore } from './AppearanceStore.svelte';
import { contrastRatio, luminance, onAccent, parseHex } from './contrast';

const base: Appearance = {
  colorScheme: 'light',
  accent: null,
  highContrast: false,
  reducedMotion: false,
  textScale: 1,
  controls: { side: 'right', buttons: ['close'] }
};

function fake(initial: Appearance) {
  const cbs = new Set<(a: Appearance) => void>();
  const gw: AppearanceGateway = {
    get: async () => initial,
    onChange: (cb) => {
      cbs.add(cb);
      return () => cbs.delete(cb);
    }
  };
  return { gw, emit: (a: Appearance) => cbs.forEach((cb) => cb(a)), cbs };
}

describe('AppearanceStore', () => {
  it('writes data attributes and accent on start', async () => {
    const root = document.createElement('html');
    const f = fake({
      ...base,
      colorScheme: 'dark',
      accent: '#1A5FB4',
      highContrast: true,
      reducedMotion: true,
      controls: { side: 'left', buttons: ['close', 'minimize'] }
    });
    await new AppearanceStore(f.gw, root).start();
    expect(root.dataset.scheme).toBe('dark');
    expect(root.dataset.contrast).toBe('more');
    expect(root.dataset.motion).toBe('reduce');
    expect(root.dataset.controls).toBe('left');
    expect(root.style.getPropertyValue('--accent')).toBe('#1A5FB4');
    expect(root.style.getPropertyValue('--on-accent')).toBe('#ffffff');
  });

  it('follows changes and clears accent when null', async () => {
    const root = document.createElement('html');
    const f = fake({ ...base, accent: '#FF0000' });
    const store = new AppearanceStore(f.gw, root);
    await store.start();
    f.emit({ ...base, colorScheme: 'dark' });
    expect(root.dataset.scheme).toBe('dark');
    expect(root.dataset.motion).toBeUndefined();
    expect(root.style.getPropertyValue('--accent')).toBe('');
    expect(store.current?.colorScheme).toBe('dark');
  });

  it('unsubscribes on stop and does not double subscribe', async () => {
    const root = document.createElement('html');
    const f = fake(base);
    const store = new AppearanceStore(f.gw, root);
    const stop = await store.start();
    await store.start();
    expect(f.cbs.size).toBe(1);
    stop();
    expect(f.cbs.size).toBe(0);
  });

  it('survives a failing get()', async () => {
    const root = document.createElement('html');
    const gw: AppearanceGateway = {
      get: () => Promise.reject(new Error('x')),
      onChange: () => () => {}
    };
    await expect(new AppearanceStore(gw, root).start()).resolves.toBeTypeOf('function');
    expect(root.dataset.scheme).toBeUndefined();
  });
});

describe('AppearanceStore text scale and motion', () => {
  // F-55 (R8): --text-scale must reach the document.
  it('review_r8_text_scale_reaches_the_document', async () => {
    const root = document.createElement('html');
    await new AppearanceStore(fake({ ...base, textScale: 1.25 }).gw, root).start();
    expect(root.style.getPropertyValue('--text-scale')).toBe('1.25');
  });

  // D-031: reduce sets the attribute, anything else removes it (OS media query wins).
  it('removes data-motion when reduced motion is off', async () => {
    const root = document.createElement('html');
    const f = fake({ ...base, reducedMotion: true });
    await new AppearanceStore(f.gw, root).start();
    expect(root.dataset.motion).toBe('reduce');
    f.emit({ ...base, reducedMotion: false });
    expect(root.dataset.motion).toBeUndefined();
  });
});

describe('onAccent', () => {
  it('picks readable text', () => {
    expect(onAccent('#3584E4')).toBe('#000000'); // 4.0:1 with white, under 4.5
    expect(onAccent('#1A5FB4')).toBe('#ffffff');
    expect(onAccent('#000000')).toBe('#ffffff');
    expect(onAccent('#FFFFFF')).toBe('#000000');
    expect(onAccent('#F6D32D')).toBe('#000000'); // yellow
    expect(onAccent('#33D17A')).toBe('#000000'); // green
    expect(onAccent('bad')).toBe('#ffffff');
  });
  it('math helpers', () => {
    expect(parseHex('#0a0B0c')).toEqual([10, 11, 12]);
    expect(parseHex('#abc')).toBeNull();
    expect(luminance([255, 255, 255])).toBeCloseTo(1);
    expect(contrastRatio(1, 0)).toBeCloseTo(21);
  });
});
