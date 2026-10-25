import { describe, expect, it } from 'vitest';
import { etaBucket, formatBytes } from './format';

describe('formatBytes', () => {
  it.each([
    [0, '0 B'],
    [999, '999 B'],
    [1000, '1 KB'],
    [1500, '1.5 KB'],
    [12_345, '12 KB'],
    [62_000_000, '62 MB'],
    [9_950_000, '9.9 MB'],
    [2_500_000_000, '2.5 GB'],
    [-1, '0 B'],
    [Number.NaN, '0 B']
  ])('%d -> %s', (n, out) => {
    expect(formatBytes(n)).toBe(out);
  });
});

describe('etaBucket', () => {
  it.each([
    [null, { kind: 'unknown' }],
    [-3, { kind: 'unknown' }],
    [0, { kind: 'seconds' }],
    [45, { kind: 'seconds' }],
    [46, { kind: 'minute' }],
    [90, { kind: 'minute' }],
    [91, { kind: 'minutes', n: 2 }],
    [180, { kind: 'minutes', n: 3 }]
  ])('%s -> %j', (secs, out) => {
    expect(etaBucket(secs)).toEqual(out);
  });
});
