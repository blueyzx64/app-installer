import { describe, expect, it } from 'vitest';
import { appIdFromName } from './app-id';

describe('appIdFromName', () => {
  it('lower-cases and joins runs with one hyphen', () => {
    expect(appIdFromName('Lumen  Notes_2')).toBe('lumen-notes-2');
  });
  it('trims hyphens and falls back', () => {
    expect(appIdFromName('--x--')).toBe('x');
    expect(appIdFromName('日本')).toBe('app');
  });
  it('truncates to 64', () => {
    expect(appIdFromName('a'.repeat(100))).toHaveLength(64);
  });
});
