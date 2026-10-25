// Pure reducer for the first-launch "make default" sheet. See docs/architecture/state-machines.md.
import type { DefaultStatus } from './types';

export type DefaultPromptState =
  | { tag: 'checking' }
  | { tag: 'hidden' }
  | { tag: 'prompting' }
  | { tag: 'applying' }
  | { tag: 'confirmed' };

export type DefaultPromptEvent =
  | { type: 'STATUS'; status: DefaultStatus }
  | { type: 'STATUS_FAILED' }
  | { type: 'MAKE_DEFAULT' }
  | { type: 'NOT_NOW' }
  | { type: 'APPLIED' }
  | { type: 'APPLY_FAILED' }
  | { type: 'TOAST_DONE' };

/** How long the confirmation toast stays (controller owns the timer). */
export const CONFIRMED_MS = 4000;

export const initialDefaultPromptState: DefaultPromptState = { tag: 'checking' };

export function defaultPromptMachine(
  state: DefaultPromptState,
  event: DefaultPromptEvent
): DefaultPromptState {
  switch (state.tag) {
    case 'checking':
      if (event.type === 'STATUS') {
        const { isDefault, askedBefore } = event.status;
        return isDefault || askedBefore ? { tag: 'hidden' } : { tag: 'prompting' };
      }
      if (event.type === 'STATUS_FAILED') return { tag: 'hidden' };
      return state;
    case 'prompting':
      if (event.type === 'MAKE_DEFAULT') return { tag: 'applying' };
      if (event.type === 'NOT_NOW') return { tag: 'hidden' };
      return state;
    case 'applying':
      if (event.type === 'APPLIED') return { tag: 'confirmed' };
      if (event.type === 'APPLY_FAILED') return { tag: 'hidden' }; // silent, logged
      return state;
    case 'confirmed':
      return event.type === 'TOAST_DONE' ? { tag: 'hidden' } : state;
    case 'hidden':
      return state;
  }
}
