// Pure reducer for the installed-apps library. See docs/architecture/state-machines.md.
import type { UninstallPlan } from '../ports/library-gateway';
import type { CommandError, InstalledApp, UninstallOutcome } from './types';

/** Plain-language toast payload; null when there is nothing to say. */
export type LibraryNotice = CommandError | null;

export type LibraryState =
  | { tag: 'loading' }
  | { tag: 'empty'; notice: LibraryNotice }
  | { tag: 'listing'; apps: InstalledApp[]; notice: LibraryNotice }
  /** `plan` is null until the gateway answers; 'failed' = it could not. CONFIRM waits for either. */
  | {
      tag: 'confirming';
      apps: InstalledApp[];
      app: InstalledApp;
      plan: UninstallPlan | 'failed' | null;
    }
  | { tag: 'removing'; apps: InstalledApp[]; app: InstalledApp };

export type LibraryEvent =
  | { type: 'LOADED'; apps: InstalledApp[] }
  | { type: 'LOAD_FAILED'; error: CommandError }
  | { type: 'TRASH'; appId: string }
  | { type: 'PLAN_LOADED'; appId: string; plan: UninstallPlan }
  | { type: 'PLAN_FAILED'; appId: string }
  | { type: 'CANCEL' }
  | { type: 'CONFIRM' }
  | { type: 'UNINSTALL_DONE'; outcome: UninstallOutcome }
  | { type: 'UNINSTALL_FAILED'; error: CommandError }
  | { type: 'DISMISS_NOTICE' };

export const initialLibraryState: LibraryState = { tag: 'loading' };

export function appsOf(s: LibraryState): InstalledApp[] {
  return 'apps' in s ? s.apps : [];
}

function listOf(apps: InstalledApp[], notice: LibraryNotice = null): LibraryState {
  return apps.length === 0 ? { tag: 'empty', notice } : { tag: 'listing', apps, notice };
}

export function libraryMachine(state: LibraryState, event: LibraryEvent): LibraryState {
  switch (state.tag) {
    case 'loading':
      if (event.type === 'LOADED') return listOf(event.apps);
      if (event.type === 'LOAD_FAILED') return { tag: 'empty', notice: event.error };
      return state;

    case 'empty':
      if (event.type === 'LOADED') return listOf(event.apps);
      if (event.type === 'DISMISS_NOTICE') return { tag: 'empty', notice: null };
      return state;

    case 'listing': {
      if (event.type === 'TRASH') {
        const app = state.apps.find((a) => a.id === event.appId);
        return app ? { tag: 'confirming', apps: state.apps, app, plan: null } : state;
      }
      if (event.type === 'LOADED') return listOf(event.apps, state.notice);
      if (event.type === 'DISMISS_NOTICE') return { ...state, notice: null };
      return state;
    }

    case 'confirming':
      if (event.type === 'CANCEL') return listOf(state.apps);
      if (event.type === 'PLAN_LOADED' && event.appId === state.app.id) {
        return { ...state, plan: event.plan };
      }
      if (event.type === 'PLAN_FAILED' && event.appId === state.app.id) {
        return { ...state, plan: 'failed' };
      }
      if (event.type === 'CONFIRM' && state.plan) {
        return { tag: 'removing', apps: state.apps, app: state.app };
      }
      return state;

    case 'removing': {
      const rest = state.apps.filter((a) => a.id !== state.app.id);
      if (event.type === 'UNINSTALL_DONE') {
        const o = event.outcome;
        if (o.kind === 'removed') return listOf(rest);
        if (o.kind === 'cancelled') return listOf(state.apps); // cancel is never an error
        return listOf(state.apps, { code: o.code, detail: o.detail });
      }
      if (event.type === 'UNINSTALL_FAILED') return listOf(state.apps, event.error);
      return state;
    }
  }
}
