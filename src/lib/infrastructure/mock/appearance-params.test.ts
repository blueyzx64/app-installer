import { describe, expect, it } from 'vitest';
import type { Appearance } from '../../core/domain/types';
import { appearanceFromSearch } from './appearance-params';

const base: Appearance = {
  colorScheme: 'light',
  accent: null,
  highContrast: false,
  reducedMotion: false,
  controls: { side: 'right', buttons: ['close'] }
};

describe('appearanceFromSearch', () => {
  it('keeps defaults with no params', () => {
    expect(appearanceFromSearch('', base)).toEqual(base);
  });
  it('mac layout', () => {
    const a = appearanceFromSearch('?side=left&buttons=close,minimize,maximize', base);
    expect(a.controls).toEqual({ side: 'left', buttons: ['close', 'minimize', 'maximize'] });
  });
  it('ignores junk and duplicates', () => {
    const a = appearanceFromSearch('?side=up&buttons=close,close,fly', base);
    expect(a.controls).toEqual({ side: 'right', buttons: ['close'] });
  });
  it('scheme', () => {
    expect(appearanceFromSearch('?scheme=dark', base).colorScheme).toBe('dark');
  });
});
