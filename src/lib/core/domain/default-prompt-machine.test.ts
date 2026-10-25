import { describe, expect, it } from 'vitest';
import {
  defaultPromptMachine as m,
  initialDefaultPromptState,
  type DefaultPromptState
} from './default-prompt-machine';

const st = (isDefault: boolean, askedBefore: boolean) => ({
  type: 'STATUS' as const,
  status: { isDefault, askedBefore }
});
const prompting: DefaultPromptState = { tag: 'prompting' };

describe('defaultPromptMachine', () => {
  it('checking -> hidden when already default or asked', () => {
    expect(m(initialDefaultPromptState, st(true, false))).toEqual({ tag: 'hidden' });
    expect(m(initialDefaultPromptState, st(false, true))).toEqual({ tag: 'hidden' });
    expect(m(initialDefaultPromptState, st(true, true))).toEqual({ tag: 'hidden' });
  });
  it('checking -> prompting when neither', () => {
    expect(m(initialDefaultPromptState, st(false, false))).toEqual(prompting);
  });
  it('status failure hides the sheet', () => {
    expect(m(initialDefaultPromptState, { type: 'STATUS_FAILED' })).toEqual({ tag: 'hidden' });
  });
  it('prompting -> applying -> confirmed -> hidden', () => {
    const a = m(prompting, { type: 'MAKE_DEFAULT' });
    expect(a).toEqual({ tag: 'applying' });
    const c = m(a, { type: 'APPLIED' });
    expect(c).toEqual({ tag: 'confirmed' });
    expect(m(c, { type: 'TOAST_DONE' })).toEqual({ tag: 'hidden' });
  });
  it('applying + error -> hidden (silent)', () => {
    expect(m({ tag: 'applying' }, { type: 'APPLY_FAILED' })).toEqual({ tag: 'hidden' });
  });
  it('prompting + NOT_NOW -> hidden', () => {
    expect(m(prompting, { type: 'NOT_NOW' })).toEqual({ tag: 'hidden' });
  });
  it('hidden is terminal; other events are ignored', () => {
    const h: DefaultPromptState = { tag: 'hidden' };
    for (const e of [st(false, false), { type: 'MAKE_DEFAULT' }, { type: 'NOT_NOW' }] as const) {
      expect(m(h, e)).toBe(h);
    }
    expect(m(prompting, { type: 'APPLIED' })).toBe(prompting);
    expect(m(initialDefaultPromptState, { type: 'MAKE_DEFAULT' })).toBe(initialDefaultPromptState);
  });
});
