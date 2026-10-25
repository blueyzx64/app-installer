import { describe, expect, it } from 'vitest';
import { createMockDefaults, defaultStatusFor, parseDefaultScenario } from './defaults-gateway';

describe('mock defaults scenarios', () => {
  it('parses ?default=', () => {
    expect(parseDefaultScenario('?default=yes')).toBe('yes');
    expect(parseDefaultScenario('?default=asked')).toBe('asked');
    expect(parseDefaultScenario('?default=bogus')).toBeNull();
    expect(parseDefaultScenario('')).toBeNull();
  });
  it('maps to status', () => {
    expect(defaultStatusFor('yes')).toEqual({ isDefault: true, askedBefore: true });
    expect(defaultStatusFor('asked')).toEqual({ isDefault: false, askedBefore: true });
    expect(defaultStatusFor('no')).toEqual({ isDefault: false, askedBefore: false });
    expect(defaultStatusFor(null)).toEqual({ isDefault: false, askedBefore: false });
  });
  it('make and dismiss update status', async () => {
    const g = createMockDefaults(defaultStatusFor('no'));
    await g.dismissPrompt();
    expect((await g.status()).askedBefore).toBe(true);
    await g.makeDefault();
    expect((await g.status()).isDefault).toBe(true);
  });
});
