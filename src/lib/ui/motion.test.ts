// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { isReducedMotion, motionMs } from './motion';

afterEach(() => {
  delete document.documentElement.dataset.motion;
});

describe('isReducedMotion', () => {
  it('is true when data-motion=reduce', () => {
    document.documentElement.dataset.motion = 'reduce';
    expect(isReducedMotion()).toBe(true);
  });

  it('is false when data-motion=full', () => {
    document.documentElement.dataset.motion = 'full';
    expect(isReducedMotion()).toBe(false);
  });
});

describe('motionMs', () => {
  it('returns 120 for 500 when reduced', () => {
    document.documentElement.dataset.motion = 'reduce';
    expect(motionMs(500)).toBe(120);
  });

  it('returns 500 for 500 when full', () => {
    document.documentElement.dataset.motion = 'full';
    expect(motionMs(500)).toBe(500);
  });
});
