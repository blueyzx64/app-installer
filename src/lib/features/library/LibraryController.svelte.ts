import { extrasCopy, removeErrorCopy, text, trashCopy } from '$lib/core/copy/copy';
import {
  appsOf,
  initialLibraryState,
  libraryMachine,
  type LibraryEvent,
  type LibraryState
} from '$lib/core/domain/library-machine';
import { toCommandError, type InstallEvent, type InstalledApp } from '$lib/core/domain/types';
import type { Gateways, UninstallPlan } from '$lib/core/ports';
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
  /** The plan has arrived, so Confirm is allowed. */
  ready: boolean;
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
    const extras = s.plan
      ? extrasCopy(s.app.displayName, s.plan === 'failed' ? 'unknown' : s.plan.alsoRemoves)
      : null;
    return {
      app: s.app,
      title: text(c.title),
      body: extras ? `${text(c.body)} ${text(extras)}` : text(c.body),
      ready: s.plan !== null,
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

  #loadSeq = 0;
  /** Newest list that arrived while the alert was up or a removal ran. Applied on return. */
  #pending: InstalledApp[] | null = null;

  async load(): Promise<void> {
    // The controller outlives the route: a slow earlier load must not overwrite a newer one.
    const seq = ++this.#loadSeq;
    try {
      const apps = await this.#gw.library.listInstalled();
      if (seq !== this.#loadSeq) return;
      const tag = this.state.tag;
      if (tag === 'confirming' || tag === 'removing') this.#pending = apps;
      else this.#dispatch({ type: 'LOADED', apps });
    } catch (e) {
      if (seq === this.#loadSeq) this.#dispatch({ type: 'LOAD_FAILED', error: toCommandError(e) });
    }
  }

  async requestTrash(appId: string): Promise<void> {
    this.#dispatch({ type: 'TRASH', appId });
    const s = this.state;
    if (s.tag !== 'confirming' || s.app.id !== appId) return;
    try {
      const plan: UninstallPlan = await this.#gw.library.uninstallPlan(appId);
      this.#dispatch({ type: 'PLAN_LOADED', appId, plan });
    } catch {
      // Not knowing is not zero: the alert warns generically and the person may still go on.
      this.#dispatch({ type: 'PLAN_FAILED', appId });
    }
  }

  cancelTrash(): void {
    this.#dispatch({ type: 'CANCEL' });
    this.#applyPending(null);
  }

  /** Apply a list that arrived during the alert or removal, minus an app just removed. */
  #applyPending(removedId: string | null): void {
    const apps = this.#pending;
    this.#pending = null;
    if (!apps || this.state.tag === 'confirming' || this.state.tag === 'removing') return;
    this.#dispatch({ type: 'LOADED', apps: apps.filter((a) => a.id !== removedId) });
  }

  async confirmTrash(): Promise<void> {
    const s = this.state;
    if (s.tag !== 'confirming' || s.plan === null) return; // wait for the plan
    const app = s.app;
    this.removalAskingPermission = false;
    this.#dispatch({ type: 'CONFIRM' });
    const onEvent = (e: InstallEvent) => {
      if (e.kind === 'progress') this.removalAskingPermission = e.phase === 'waitingForPermission';
    };
    let removed = false;
    try {
      const outcome = await this.#gw.library.uninstall(app.id, onEvent);
      // Lists fetched while removing may predate it: never let them bring the app back.
      this.#loadSeq++;
      removed = outcome.kind === 'removed';
      if (outcome.kind === 'failed') this.#showNoticeFor(app.displayName);
      this.#dispatch({ type: 'UNINSTALL_DONE', outcome });
    } catch (e) {
      this.#loadSeq++;
      this.#showNoticeFor(app.displayName);
      this.#dispatch({ type: 'UNINSTALL_FAILED', error: toCommandError(e) });
    }
    this.#applyPending(removed ? app.id : null);
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
