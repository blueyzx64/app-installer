import { describe, expect, it } from 'vitest';
import type { Appearance } from '../../core/domain/types';
import { appearanceFromSearch } from './appearance-params';

const base: Appearance = {
  colorScheme: 'light',
  accent: null,
  highContrast: false,
  reducedMotion: false,
  textScale: 1,
  controls: { side: 'right', buttons: ['close'] }
};

describe('appearanceFromSearch', () => {
  it('keeps defaults with no params', () => {
    expect(appearanceFromSearch('', base)).toEqual({ ...base, textScale: 1 });
  });
  it('mac layout', () => {
    const a = appearanceFromSearch('?side=left&buttons=close,minimize,maximize', base);
    expect(a.controls).toEqual({ side: 'left', buttons: ['close', 'minimize', 'maximize'] });
  });
  it('ignores junk and duplicates', () => {
    const a = appearanceFromSearch('?side=up&buttons=close,close,fly', base);
    expect(a.controls).toEqual({ side: 'right', buttons: ['close'] });
  });
  it('text scale', () => {
    expect(appearanceFromSearch('?textScale=1.25', base).textScale).toBe(1.25);
    expect(appearanceFromSearch('?textScale=9', base).textScale).toBe(3); // clamped like Rust
  });
  it('scheme', () => {
    expect(appearanceFromSearch('?scheme=dark', base).colorScheme).toBe('dark');
  });

  it('motion and textScale', () => {
    expect(appearanceFromSearch('?motion=reduce', base).reducedMotion).toBe(true);
    expect(appearanceFromSearch('?motion=full', base).reducedMotion).toBe(false);
    const a = appearanceFromSearch('?textScale=1.25', base) as { textScale?: number };
    expect(a.textScale).toBe(1.25);
    const b = appearanceFromSearch('?textScale=9', base) as { textScale?: number };
    expect(b.textScale).toBe(3);
  });
});
