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
 * States with a preview may carry `source`: the file it was read from (used to never queue it twice).
 * `idle` = no package yet (initial) or window closing.
 */
export type InstallerState =
  | { tag: 'idle' }
  | { tag: 'inspecting'; path: string; queued: string[] }
  | { tag: 'ready'; preview: PackagePreview; queued: string[]; source?: string }
  | {
      tag: 'dragging';
      preview: PackagePreview;
      overFolder: boolean;
      queued: string[];
      source?: string;
    }
  | { tag: 'starting'; preview: PackagePreview; plan?: Plan; queued: string[]; source?: string }
  | {
      tag: 'awaitingPermission';
      preview: PackagePreview;
      plan?: Plan;
      queued: string[];
      source?: string;
    }
  | {
      tag: 'installing';
      preview: PackagePreview;
      phase: InstallPhase;
      percent: number | null;
      /** Highest percent seen in this phase. Survives an "unknown" (null) report. */
      peak: number;
      etaSecs: number | null;
      /** The backend accepts a cancel right now (F-28). */
      allowCancel: boolean;
      plan?: Plan;
      queued: string[];
      source?: string;
    }
  | {
      tag: 'succeeded';
      preview: PackagePreview;
      app: InstalledApp;
      queued: string[];
      source?: string;
    }
  | { tag: 'incompatible'; preview: PackagePreview; queued: string[]; source?: string }
  | {
      tag: 'failed';
      preview: PackagePreview | null;
      /** Path that failed to inspect (preview null), used by RETRY. */
      path: string | null;
      code: ErrorCode;
      detail: string | null;
      queued: string[];
      source?: string;
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
      allowCancel?: boolean;
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

/** Cancel is offered while starting or waiting for permission, then only while the backend allows it. */
export function canCancel(s: InstallerState): boolean {
  if (s.tag === 'awaitingPermission' || s.tag === 'starting') return true;
  if (s.tag === 'installing') return s.allowCancel && s.phase !== 'finishing';
  return false;
}

const PHASE_RANK: Record<InstallPhase, number> = {
  preparing: 0,
  waitingForPermission: 1,
  downloading: 2,
  installing: 3,
  finishing: 4
};

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

type Open = Exclude<InstallerState, { tag: 'idle' }>;

/** The file shown right now (being read, or the preview's source). */
function currentPath(s: Open): string | undefined {
  if (s.tag === 'inspecting') return s.path;
  if (s.tag === 'failed' && s.preview === null) return s.path ?? undefined;
  return s.source;
}

/** Queue a path, unless it is already queued or is the file on screen (no doubles, ever). */
function enqueue(s: Open, path: string): InstallerState {
  if (path === currentPath(s) || s.queued.includes(path)) return s;
  return { ...s, queued: [...s.queued, path] };
}

/** queue + source, carried across transitions. */
function keep(s: Open): { queued: string[]; source?: string } {
  return 'source' in s && s.source !== undefined
    ? { queued: s.queued, source: s.source }
    : { queued: s.queued };
}

function progressed(
  prev: Extract<InstallerState, { tag: 'installing' }> | null,
  e: Extract<InstallerEvent, { type: 'PROGRESS' }>
): { percent: number | null; peak: number; etaSecs: number | null } {
  let percent = normalizePercent(e.percent);
  let peak = percent ?? 0;
  // Never go backwards within a phase, even across an "unknown" report.
  if (prev && prev.phase === e.phase) {
    peak = Math.max(prev.peak, peak);
    if (percent !== null) percent = peak;
  }
  return { percent, peak, etaSecs: e.etaSecs };
}

export function installerMachine(state: InstallerState, event: InstallerEvent): InstallerState {
  // OPEN is handled first: it works from (almost) every state.
  if (event.type === 'OPEN') {
    if (state.tag === 'idle') return { tag: 'inspecting', path: event.path, queued: [] };
    // Never replace the preview while a drag/install is in flight.
    if (state.tag === 'dragging' || isBusy(state)) return enqueue(state, event.path);
    // Keep the file being inspected; a second one waits its turn (never dropped, never doubled).
    if (state.tag === 'inspecting') return enqueue(state, event.path);
    // Replacing the screen: the new file leaves the queue so it is not opened twice.
    return {
      tag: 'inspecting',
      path: event.path,
      queued: state.queued.filter((q) => q !== event.path)
    };
  }

  switch (state.tag) {
    case 'idle':
      return state;

    case 'inspecting': {
      if (event.type === 'INSPECTED') {
        if (event.path !== undefined && event.path !== state.path) return state; // stale
        const preview = event.preview;
        return isInstallable(preview)
          ? { tag: 'ready', preview, queued: state.queued, source: state.path }
          : { tag: 'incompatible', preview, queued: state.queued, source: state.path };
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
        return { tag: 'dragging', preview: state.preview, overFolder: false, ...keep(state) };
      }
      if (event.type === 'KEY_INSTALL') {
        return { tag: 'starting', preview: state.preview, ...keep(state) };
      }
      if (event.type === 'CLOSE') return close(state);
      return state;
    }

    case 'dragging': {
      if (event.type === 'DRAG_OVER') return { ...state, overFolder: event.overFolder };
      if (event.type === 'DRAG_CANCEL') {
        return { tag: 'ready', preview: state.preview, ...keep(state) };
      }
      if (event.type === 'DROP_ON_FOLDER') {
        return { tag: 'starting', preview: state.preview, ...keep(state) };
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
            ...keep(state)
          };
        }
        const p = progressed(null, event);
        return {
          tag: 'installing',
          preview: state.preview,
          phase: event.phase,
          percent: p.percent,
          peak: p.peak,
          etaSecs: p.etaSecs,
          allowCancel: event.allowCancel ?? false,
          plan: state.plan,
          ...keep(state)
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
        // A late event never moves the phase backwards.
        if (PHASE_RANK[event.phase] < PHASE_RANK[state.phase]) return state;
        const p = progressed(state, event);
        return {
          ...state,
          phase: event.phase,
          percent: p.percent,
          peak: p.peak,
          etaSecs: p.etaSecs,
          allowCancel: event.allowCancel ?? false
        };
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
          return { tag: 'ready', preview: state.preview, ...keep(state) };
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
      return { tag: 'succeeded', preview: state.preview, app: outcome.app, ...keep(state) };
    case 'cancelled':
      return { tag: 'ready', preview: state.preview, ...keep(state) };
    case 'failed':
      return {
        tag: 'failed',
        preview: state.preview,
        path: null,
        code: outcome.code,
        detail: outcome.detail,
        ...keep(state)
      };
  }
}
