import { removeErrorCopy, text, trashCopy } from '$lib/core/copy/copy';
import {
  appsOf,
  initialLibraryState,
  libraryMachine,
  type LibraryEvent,
  type LibraryState
} from '$lib/core/domain/library-machine';
import { toCommandError, type InstallEvent, type InstalledApp } from '$lib/core/domain/types';
import type { Gateways } from '$lib/core/ports';
import { t } from '$lib/i18n';

/** How long a failure toast stays before it hides itself. */
export const NOTICE_MS = 5000;

export interface ConfirmView {
  app: InstalledApp;
  title: string;
  body: string;
  confirmLabel: string;
  cancelLabel: string;
  system: boolean;
}

export class LibraryController {
  state = $state.raw<LibraryState>(initialLibraryState);
  /** Latest phase reported by an uninstall. */
  removalAskingPermission = $state(false);

  readonly #gw: Gateways;
  #noticeApp: string | null = null;
  #noticeTimer: ReturnType<typeof setTimeout> | null = null;

  constructor(gateways: Gateways) {
    this.#gw = gateways;
  }

  apps = $derived(appsOf(this.state));

  confirm = $derived.by<ConfirmView | null>(() => {
    const s = this.state;
    if (s.tag !== 'confirming') return null;
    const c = trashCopy(s.app);
    return {
      app: s.app,
      title: text(c.title),
      body: text(c.body),
      confirmLabel: text(c.confirm),
      cancelLabel: text(c.cancel),
      system: c.system
    };
  });

  removingId = $derived(this.state.tag === 'removing' ? this.state.app.id : null);

  removalText = $derived.by(() => {
    const s = this.state;
    if (s.tag !== 'removing') return '';
    return this.removalAskingPermission
      ? t('perm.waiting')
      : t('library.removing', { app: s.app.displayName });
  });

  /** Plain-language toast text for a failed removal. null = nothing to show. */
  notice = $derived.by(() => {
    const s = this.state;
    if (s.tag === 'loading' || s.tag === 'confirming' || s.tag === 'removing') return null;
    if (!s.notice || this.#noticeApp === null) return null;
    return text(removeErrorCopy(s.notice.code, this.#noticeApp));
  });

  async load(): Promise<void> {
    try {
      const apps = await this.#gw.library.listInstalled();
      this.#dispatch({ type: 'LOADED', apps });
    } catch (e) {
      this.#dispatch({ type: 'LOAD_FAILED', error: toCommandError(e) });
    }
  }

  requestTrash(appId: string): void {
    this.#dispatch({ type: 'TRASH', appId });
  }

  cancelTrash(): void {
    this.#dispatch({ type: 'CANCEL' });
  }

  async confirmTrash(): Promise<void> {
    const s = this.state;
    if (s.tag !== 'confirming') return;
    const app = s.app;
    this.removalAskingPermission = false;
    this.#dispatch({ type: 'CONFIRM' });
    const onEvent = (e: InstallEvent) => {
      if (e.kind === 'progress') this.removalAskingPermission = e.phase === 'waitingForPermission';
    };
    try {
      const outcome = await this.#gw.library.uninstall(app.id, onEvent);
      if (outcome.kind === 'failed') this.#showNoticeFor(app.displayName);
      this.#dispatch({ type: 'UNINSTALL_DONE', outcome });
    } catch (e) {
      this.#showNoticeFor(app.displayName);
      this.#dispatch({ type: 'UNINSTALL_FAILED', error: toCommandError(e) });
    }
    this.removalAskingPermission = false;
  }

  dismissNotice(): void {
    this.#clearNoticeTimer();
    this.#noticeApp = null;
    this.#dispatch({ type: 'DISMISS_NOTICE' });
  }

  async launch(appId: string): Promise<void> {
    try {
      await this.#gw.library.launchApp(appId);
    } catch {
      // Launch failures are silent in v1.
    }
  }

  #showNoticeFor(appName: string): void {
    this.#noticeApp = appName;
    this.#clearNoticeTimer();
    this.#noticeTimer = setTimeout(() => this.dismissNotice(), NOTICE_MS);
  }

  #clearNoticeTimer(): void {
    if (this.#noticeTimer) clearTimeout(this.#noticeTimer);
    this.#noticeTimer = null;
  }

  #dispatch(event: LibraryEvent): void {
    this.state = libraryMachine(this.state, event);
  }
}
