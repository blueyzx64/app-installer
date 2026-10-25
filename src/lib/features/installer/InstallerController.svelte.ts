// Holds the installer $state and dispatches to the pure machine. Side effects (gateway calls)
// are started from state transitions. Views call methods and read fields only.
import { compatCopy, errorCopy, etaCopy, phaseCopy, text } from '$lib/core/copy/copy';
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
import {
  toCommandError,
  type ErrorCode,
  type InstallEvent,
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

export interface ErrorView {
  message: string;
  action: 'close' | 'tryAgain' | null;
  code: ErrorCode;
  detail: string | null;
}

export class InstallerController {
  state = $state.raw<InstallerState>(initialInstallerState);
  launching = $state(false);

  readonly #gw: Gateways;
  #closeTimer: ReturnType<typeof setTimeout> | null = null;

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

  percent = $derived<number | null>(this.state.tag === 'installing' ? this.state.percent : null);

  phaseText = $derived.by(() => {
    const s = this.state;
    if (s.tag === 'awaitingPermission') return t('perm.waiting');
    if (s.tag === 'starting') return t('phase.preparing');
    if (s.tag === 'installing') {
      return text(phaseCopy(s.phase, { app: s.preview.displayName, plan: s.plan }));
    }
    return '';
  });

  etaText = $derived(this.state.tag === 'installing' ? text(etaCopy(this.state.etaSecs)) : '');

  incompatible = $derived.by<IncompatibleView | null>(() => {
    const s = this.state;
    if (s.tag !== 'incompatible') return null;
    const c = compatCopy(s.preview.compat, s.preview.displayName);
    return c
      ? { title: text(c.title), body: c.body ? text(c.body) : null, action: c.action }
      : null;
  });

  error = $derived.by<ErrorView | null>(() => {
    const s = this.state;
    if (s.tag !== 'failed') return null;
    const c = errorCopy(s.code, { app: s.preview?.displayName ?? '' });
    return { message: text(c.message), action: c.action, code: s.code, detail: s.detail };
  });

  /** Subscribe to files opened while running. Call in an $effect and return the cleanup. */
  start(): () => void {
    const off = this.#gw.installer.onOpenFile((path) => this.open(path));
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

  cancelInstall(): void {
    if (!this.canCancel) return;
    const p = previewOf(this.state);
    if (!p) return;
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

  /** Incompatible (already installed, same version) "Open". */
  openInstalled(): void {
    const s = this.state;
    if (s.tag !== 'incompatible' || this.launching) return;
    const name = s.preview.displayName;
    this.launching = true;
    this.#gw.library
      .listInstalled()
      .then((apps) => {
        const app = apps.find((a) => a.displayName === name);
        if (app) this.#launchThenClose(app.id);
        else this.launching = false;
      })
      .catch(() => {
        this.launching = false;
      });
  }

  #launchThenClose(appId: string): void {
    this.launching = true;
    this.#gw.library.launchApp(appId).catch(() => {});
    this.#clearTimer();
    this.#closeTimer = setTimeout(() => {
      this.#closeTimer = null;
      this.launching = false;
      this.close();
    }, OPEN_APP_CLOSE_MS);
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
          etaSecs: e.etaSecs
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
