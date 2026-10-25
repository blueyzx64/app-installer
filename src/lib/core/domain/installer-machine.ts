// Pure reducer for the installer window. See docs/architecture/state-machines.md.
// No timers, IO or Svelte. Controllers translate gestures/gateway callbacks into events.
import type {
  ErrorCode,
  InstallOutcome,
  InstallPhase,
  InstalledApp,
  PackagePreview,
  Plan
} from './types';

/**
 * Every state except `idle` carries `queued`: paths opened while busy (FIFO).
 * `idle` = no package yet (initial) or window closing.
 */
export type InstallerState =
  | { tag: 'idle' }
  | { tag: 'inspecting'; path: string; queued: string[] }
  | { tag: 'ready'; preview: PackagePreview; queued: string[] }
  | { tag: 'dragging'; preview: PackagePreview; overFolder: boolean; queued: string[] }
  | { tag: 'starting'; preview: PackagePreview; plan?: Plan; queued: string[] }
  | { tag: 'awaitingPermission'; preview: PackagePreview; plan?: Plan; queued: string[] }
  | {
      tag: 'installing';
      preview: PackagePreview;
      phase: InstallPhase;
      percent: number | null;
      etaSecs: number | null;
      plan?: Plan;
      queued: string[];
    }
  | { tag: 'succeeded'; preview: PackagePreview; app: InstalledApp; queued: string[] }
  | { tag: 'incompatible'; preview: PackagePreview; queued: string[] }
  | {
      tag: 'failed';
      preview: PackagePreview | null;
      /** Path that failed to inspect (preview null), used by RETRY. */
      path: string | null;
      code: ErrorCode;
      detail: string | null;
      queued: string[];
    };

export type InstallerTag = InstallerState['tag'];

export type InstallerEvent =
  | { type: 'OPEN'; path: string }
  | { type: 'INSPECTED'; path?: string; preview: PackagePreview }
  | { type: 'INSPECT_FAILED'; path?: string; code: ErrorCode; detail: string | null }
  | { type: 'DRAG_START' }
  | { type: 'DRAG_OVER'; overFolder: boolean }
  | { type: 'DRAG_CANCEL' }
  | { type: 'DROP_ON_FOLDER' }
  | { type: 'KEY_INSTALL' }
  | { type: 'PLAN'; plan: Plan }
  | {
      type: 'PROGRESS';
      phase: InstallPhase;
      percent: number | null;
      etaSecs: number | null;
    }
  | { type: 'FINISHED'; outcome: InstallOutcome }
  | { type: 'RETRY' }
  | { type: 'CLOSE' };

export const initialInstallerState: InstallerState = { tag: 'idle' };

/** States the user can't leave by gesture (only by Cancel / backend outcome). */
export const BUSY_TAGS = ['starting', 'awaitingPermission', 'installing'] as const;

export function isBusy(s: InstallerState): boolean {
  return s.tag === 'starting' || s.tag === 'awaitingPermission' || s.tag === 'installing';
}

/** Cancel is offered while waiting for permission or before the final phase. */
export function canCancel(s: InstallerState): boolean {
  if (s.tag === 'awaitingPermission' || s.tag === 'starting') return true;
  if (s.tag === 'installing') return s.phase !== 'finishing';
  return false;
}

/** alreadyInstalled.newer (package newer than installed) means Update. */
export function isUpdate(p: PackagePreview | null | undefined): boolean {
  return p?.compat.kind === 'alreadyInstalled' && p.compat.relation === 'newer';
}

/** Compat that blocks install. `ok` and `alreadyInstalled.newer` are installable. */
export function isInstallable(p: PackagePreview): boolean {
  return p.compat.kind === 'ok' || isUpdate(p);
}

export function previewOf(s: InstallerState): PackagePreview | null {
  return 'preview' in s ? s.preview : null;
}

/** Clamp to 0..100; PackageKit "unknown" (101+) and non-finite become null. */
export function normalizePercent(p: number | null): number | null {
  if (p === null || !Number.isFinite(p) || p > 100) return null;
  return Math.max(0, Math.min(100, p));
}

function enqueue(s: Exclude<InstallerState, { tag: 'idle' }>, path: string): InstallerState {
  return { ...s, queued: [...s.queued, path] };
}

function progressed(
  prev: Extract<InstallerState, { tag: 'installing' }> | null,
  e: Extract<InstallerEvent, { type: 'PROGRESS' }>
): { percent: number | null; etaSecs: number | null } {
  let percent = normalizePercent(e.percent);
  // Never go backwards within a phase.
  if (prev && prev.phase === e.phase && prev.percent !== null && percent !== null) {
    percent = Math.max(prev.percent, percent);
  }
  return { percent, etaSecs: e.etaSecs };
}

export function installerMachine(state: InstallerState, event: InstallerEvent): InstallerState {
  // OPEN is handled first: it works from (almost) every state.
  if (event.type === 'OPEN') {
    if (state.tag === 'idle') return { tag: 'inspecting', path: event.path, queued: [] };
    // Never replace the preview while a drag/install is in flight.
    if (state.tag === 'dragging' || isBusy(state)) return enqueue(state, event.path);
    return { tag: 'inspecting', path: event.path, queued: state.queued };
  }

  switch (state.tag) {
    case 'idle':
      return state;

    case 'inspecting': {
      if (event.type === 'INSPECTED') {
        if (event.path !== undefined && event.path !== state.path) return state; // stale
        const preview = event.preview;
        return isInstallable(preview)
          ? { tag: 'ready', preview, queued: state.queued }
          : { tag: 'incompatible', preview, queued: state.queued };
      }
      if (event.type === 'INSPECT_FAILED') {
        if (event.path !== undefined && event.path !== state.path) return state; // stale
        return {
          tag: 'failed',
          preview: null,
          path: state.path,
          code: event.code,
          detail: event.detail,
          queued: state.queued
        };
      }
      return state;
    }

    case 'ready': {
      if (event.type === 'DRAG_START') {
        return { tag: 'dragging', preview: state.preview, overFolder: false, queued: state.queued };
      }
      if (event.type === 'KEY_INSTALL') {
        return { tag: 'starting', preview: state.preview, queued: state.queued };
      }
      if (event.type === 'CLOSE') return close(state);
      return state;
    }

    case 'dragging': {
      if (event.type === 'DRAG_OVER') return { ...state, overFolder: event.overFolder };
      if (event.type === 'DRAG_CANCEL') {
        return { tag: 'ready', preview: state.preview, queued: state.queued };
      }
      if (event.type === 'DROP_ON_FOLDER') {
        return { tag: 'starting', preview: state.preview, queued: state.queued };
      }
      return state;
    }

    case 'starting':
    case 'awaitingPermission': {
      if (event.type === 'PLAN') return { ...state, plan: event.plan };
      if (event.type === 'PROGRESS') {
        if (event.phase === 'waitingForPermission') {
          return {
            tag: 'awaitingPermission',
            preview: state.preview,
            plan: state.plan,
            queued: state.queued
          };
        }
        const p = progressed(null, event);
        return {
          tag: 'installing',
          preview: state.preview,
          phase: event.phase,
          percent: p.percent,
          etaSecs: p.etaSecs,
          plan: state.plan,
          queued: state.queued
        };
      }
      if (event.type === 'FINISHED') return finish(state, event.outcome);
      return state;
    }

    case 'installing': {
      if (event.type === 'PLAN') return { ...state, plan: event.plan };
      if (event.type === 'PROGRESS') {
        // Installing never goes back to awaiting permission.
        if (event.phase === 'waitingForPermission') return state;
        const p = progressed(state, event);
        return { ...state, phase: event.phase, percent: p.percent, etaSecs: p.etaSecs };
      }
      if (event.type === 'FINISHED') return finish(state, event.outcome);
      return state;
    }

    case 'succeeded':
      return event.type === 'CLOSE' ? close(state) : state;

    case 'incompatible':
      return event.type === 'CLOSE' ? close(state) : state;

    case 'failed': {
      if (event.type === 'RETRY') {
        if (state.preview) {
          return { tag: 'ready', preview: state.preview, queued: state.queued };
        }
        if (state.path !== null) {
          return { tag: 'inspecting', path: state.path, queued: state.queued };
        }
        return state;
      }
      if (event.type === 'CLOSE') return close(state);
      return state;
    }
  }
}

/** CLOSE: next queued package if any, else idle (the window closes). */
function close(state: Exclude<InstallerState, { tag: 'idle' }>): InstallerState {
  const [next, ...rest] = state.queued;
  return next === undefined ? { tag: 'idle' } : { tag: 'inspecting', path: next, queued: rest };
}

function finish(
  state: Extract<InstallerState, { tag: 'starting' | 'awaitingPermission' | 'installing' }>,
  outcome: InstallOutcome
): InstallerState {
  switch (outcome.kind) {
    case 'installed':
      return { tag: 'succeeded', preview: state.preview, app: outcome.app, queued: state.queued };
    case 'cancelled':
      return { tag: 'ready', preview: state.preview, queued: state.queued };
    case 'failed':
      return {
        tag: 'failed',
        preview: state.preview,
        path: null,
        code: outcome.code,
        detail: outcome.detail,
        queued: state.queued
      };
  }
}
