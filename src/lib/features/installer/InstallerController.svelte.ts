// Holds the installer $state and dispatches to the pure machine. Side effects (gateway calls)
// are started from state transitions. Views call methods and read fields only.
import {
  compatCopy,
  compatKind,
  errorCopy,
  etaCopy,
  phaseCopy,
  text,
  type CompatKind
} from '$lib/core/copy/copy';
import {
  canCancel,
  initialInstallerState,
  installerMachine,
  isBusy,
  isUpdate,
  previewOf,
  type InstallerEvent,
  type InstallerState
} from '$lib/core/domain/installer-machine';
import { formatBytes, BIG_DOWNLOAD_BYTES } from '$lib/core/domain/format';
import {
  toCommandError,
  type ErrorCode,
  type InstallEvent,
  type InstallPhase,
  type PackagePreview
} from '$lib/core/domain/types';
import type { Gateways } from '$lib/core/ports';
import { t } from '$lib/i18n';

/** How long the "Opening…" label shows before the window closes (prototype: 1100 ms). */
export const OPEN_APP_CLOSE_MS = 1100;

export interface IncompatibleView {
  title: string;
  body: string | null;
  action: 'open' | null;
}

/** Queue toast text, plain language. */
export interface QueueNotice {
  title: string;
  body: string;
}

export interface ErrorView {
  message: string;
  action: 'close' | 'tryAgain' | null;
  code: ErrorCode;
  detail: string | null;
}

function queuedOf(s: InstallerState): number {
  return s.tag === 'idle' ? 0 : s.queued.length;
}

export class InstallerController {
  state = $state.raw<InstallerState>(initialInstallerState);
  launching = $state(false);
  /** Set when a package was opened while busy and got queued; the route shows a toast. */
  queueNotice = $state<QueueNotice | null>(null);

  readonly #gw: Gateways;
  #closeTimer: ReturnType<typeof setTimeout> | null = null;
  #cancelPending = false;

  constructor(gateways: Gateways) {
    this.#gw = gateways;
  }

  preview = $derived<PackagePreview | null>(previewOf(this.state));
  appName = $derived(this.preview?.displayName ?? '');
  isUpdate = $derived(isUpdate(this.preview));
  busy = $derived(isBusy(this.state));
  canCancel = $derived(canCancel(this.state));
  dragOverFolder = $derived(this.state.tag === 'dragging' && this.state.overFolder);
  queuedCount = $derived(this.state.tag === 'idle' ? 0 : this.state.queued.length);

  iconUrl = $derived(this.preview?.iconDataUrl ?? null);
  /** System-wide install: the DMG screen shows the password note. */
  needsPassword = $derived(this.preview?.needsPermission ?? false);
  /** The permission shade is up. User-scope installs never show it (F-25). */
  waitingForPermission = $derived(
    this.state.tag === 'awaitingPermission' && this.state.preview.scope === 'system'
  );
  /** DMG screen visible: choose, drag, or waiting for the system dialog. */
  showDmg = $derived(
    this.state.tag === 'ready' ||
      this.state.tag === 'dragging' ||
      this.state.tag === 'starting' ||
      this.state.tag === 'awaitingPermission'
  );
  /** Phase for the progress screen. User scope never reports permission. */
  phase = $derived.by<InstallPhase>(() => {
    const s = this.state;
    if (s.tag !== 'installing') return 'preparing';
    return s.phase === 'waitingForPermission' && s.preview.scope === 'user' ? 'preparing' : s.phase;
  });
  /** The backend accepts a cancel during download/install (F-28). */
  allowCancel = $derived(this.state.tag === 'installing' && this.state.allowCancel);
  etaSecs = $derived(this.state.tag === 'installing' ? this.state.etaSecs : null);
  /** "120 MB" for big downloads only; otherwise null. */
  sizeLabel = $derived.by<string | null>(() => {
    const s = this.state;
    if (s.tag !== 'installing' || !s.plan || s.plan.downloadBytes <= BIG_DOWNLOAD_BYTES)
      return null;
    return formatBytes(s.plan.downloadBytes);
  });
  incompatibleKind = $derived<CompatKind | null>(
    this.state.tag === 'incompatible' ? compatKind(this.state.preview.compat) : null
  );
  /** The Done screen offers Open only if the app has a launcher. */
  canLaunch = $derived(this.state.tag === 'succeeded' && this.state.app.canLaunch);

  percent = $derived<number | null>(this.state.tag === 'installing' ? this.state.percent : null);

  phaseText = $derived.by(() => {
    const s = this.state;
    if (s.tag === 'awaitingPermission') {
      return t(s.preview.scope === 'system' ? 'perm.waiting' : 'phase.preparing');
    }
    if (s.tag === 'starting') return t('phase.preparing');
    if (s.tag === 'installing') {
      return text(phaseCopy(this.phase, { app: s.preview.displayName, plan: s.plan }));
    }
    return '';
  });

  etaText = $derived(
    this.state.tag === 'installing' && this.state.phase !== 'finishing'
      ? text(etaCopy(this.state.etaSecs))
      : ''
  );

  incompatible = $derived.by<IncompatibleView | null>(() => {
    const s = this.state;
    if (s.tag !== 'incompatible') return null;
    const c = compatCopy(s.preview.compat, s.preview.displayName);
    return c
      ? {
          title: text(c.title),
          body: c.body ? text(c.body) : null,
          // Open needs the installed app's id from the backend (v4); without it, hide Open.
          action: c.action === 'open' && s.preview.installedAppId === null ? null : c.action
        }
      : null;
  });

  error = $derived.by<ErrorView | null>(() => {
    const s = this.state;
    if (s.tag !== 'failed') return null;
    const c = errorCopy(s.code, { app: s.preview?.displayName ?? '' });
    return { message: text(c.message), action: c.action, code: s.code, detail: s.detail };
  });

  /** Subscribe to files opened while running. Call in an $effect and return the cleanup. */
  start(onOpened?: () => void): () => void {
    const off = this.#gw.installer.onOpenFile((path) => {
      this.open(path);
      onOpened?.();
    });
    return () => {
      off();
      this.#clearTimer();
    };
  }

  /** Consume the launch file (once). true if a package is now being opened. */
  async takeLaunch(): Promise<boolean> {
    try {
      const path = await this.#gw.installer.takeLaunchFile();
      if (!path) return false;
      this.open(path);
      return true;
    } catch {
      return false;
    }
  }

  open(path: string): void {
    this.#dispatch({ type: 'OPEN', path });
  }

  dismissQueueNotice(): void {
    this.queueNotice = null;
  }

  startDrag(): void {
    this.#dispatch({ type: 'DRAG_START' });
  }

  dragOver(overFolder: boolean): void {
    this.#dispatch({ type: 'DRAG_OVER', overFolder });
  }

  /** Pointer released: over the folder installs, anywhere else springs back. */
  endDrag(): void {
    if (this.state.tag !== 'dragging') return;
    this.#dispatch(this.state.overFolder ? { type: 'DROP_ON_FOLDER' } : { type: 'DRAG_CANCEL' });
  }

  cancelDrag(): void {
    this.#dispatch({ type: 'DRAG_CANCEL' });
  }

  keyInstall(): void {
    this.#dispatch({ type: 'KEY_INSTALL' });
  }

  /** Cancel (permission shade, or progress while the backend allows it). */
  cancelInstall(): void {
    this.cancel();
  }

  cancel(): void {
    if (!this.canCancel) return;
    const p = previewOf(this.state);
    if (!p) return;
    if (this.state.tag === 'starting') {
      // The backend may not know this install yet; send the cancel with the first progress event.
      this.#cancelPending = true;
      return;
    }
    // The state changes when the backend reports the `cancelled` outcome.
    this.#gw.installer.cancelInstall(p.id).catch(() => {});
  }

  retry(): void {
    this.#dispatch({ type: 'RETRY' });
  }

  /** Close/Done: next queued package, else close the window. */
  close(): void {
    this.#dispatch({ type: 'CLOSE' });
    if (this.state.tag === 'idle') void this.#gw.window.close().catch(() => {});
  }

  /** Done view "Open": launch, show "Opening…", then close. */
  openApp(): void {
    const s = this.state;
    if (s.tag !== 'succeeded' || this.launching) return;
    this.#launchThenClose(s.app.id);
  }

  /** Incompatible (already installed, same version) "Open": uses the backend's installedAppId. */
  openInstalled(): void {
    const s = this.state;
    if (s.tag !== 'incompatible' || this.launching) return;
    const id = s.preview.installedAppId;
    if (id === null) return;
    this.#launchThenClose(id);
  }

  #launchThenClose(appId: string): void {
    this.launching = true;
    this.#clearTimer();
    this.#closeTimer = setTimeout(() => {
      this.#closeTimer = null;
      this.launching = false;
      this.close();
    }, OPEN_APP_CLOSE_MS);
    // A failed launch keeps the window open instead of pretending the app started.
    this.#gw.library.launchApp(appId).catch(() => {
      this.#clearTimer();
      this.launching = false;
    });
  }

  #clearTimer(): void {
    if (this.#closeTimer) clearTimeout(this.#closeTimer);
    this.#closeTimer = null;
  }

  #dispatch(event: InstallerEvent): void {
    const prev = this.state;
    const next = installerMachine(prev, event);
    if (next === prev) return;
    this.state = next;
    if (event.type === 'OPEN' && next.tag !== 'idle' && queuedOf(next) > queuedOf(prev)) {
      // Queued behind a drag or an install: tell the person it will come next.
      this.queueNotice = { title: t('queue.title'), body: t('queue.body') };
    }
    if (this.#cancelPending && (next.tag === 'awaitingPermission' || next.tag === 'installing')) {
      this.#cancelPending = false;
      this.#gw.installer.cancelInstall(next.preview.id).catch(() => {});
    }
    if (!isBusy(next)) this.#cancelPending = false;
    if (next.tag === 'inspecting' && (prev.tag !== 'inspecting' || prev.path !== next.path)) {
      this.#inspect(next.path);
    } else if (next.tag === 'starting' && prev.tag !== 'starting') {
      this.#install(next.preview);
    }
  }

  #inspect(path: string): void {
    this.#gw.installer
      .openPackage(path)
      .then((preview) => this.#dispatch({ type: 'INSPECTED', path, preview }))
      .catch((e: unknown) => {
        const { code, detail } = toCommandError(e);
        this.#dispatch({ type: 'INSPECT_FAILED', path, code, detail });
      });
  }

  #install(preview: PackagePreview): void {
    const onEvent = (e: InstallEvent) => {
      if (e.kind === 'plan') {
        this.#dispatch({
          type: 'PLAN',
          plan: { extraPackages: e.extraPackages, downloadBytes: e.downloadBytes }
        });
      } else {
        this.#dispatch({
          type: 'PROGRESS',
          phase: e.phase,
          percent: e.percent,
          etaSecs: e.etaSecs,
          allowCancel: e.allowCancel
        });
      }
    };
    this.#gw.installer
      .install(preview.id, onEvent)
      .then((outcome) => this.#dispatch({ type: 'FINISHED', outcome }))
      .catch((e: unknown) => {
        const { code, detail } = toCommandError(e);
        this.#dispatch({ type: 'FINISHED', outcome: { kind: 'failed', code, detail } });
      });
  }
}
