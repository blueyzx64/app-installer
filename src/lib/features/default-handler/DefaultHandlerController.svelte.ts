import {
  CONFIRMED_MS,
  defaultPromptMachine,
  initialDefaultPromptState,
  type DefaultPromptEvent,
  type DefaultPromptState
} from '$lib/core/domain/default-prompt-machine';
import type { Gateways } from '$lib/core/ports';

export class DefaultHandlerController {
  state = $state.raw<DefaultPromptState>(initialDefaultPromptState);

  readonly #gw: Gateways;
  readonly #canShow: () => boolean;
  #toastTimer: ReturnType<typeof setTimeout> | null = null;

  /**
   * @param canShow true when the sheet may appear (the installer is `ready`: after inspection,
   *   never over the permission shade or an active install).
   */
  constructor(gateways: Gateways, canShow: () => boolean) {
    this.#gw = gateways;
    this.#canShow = canShow;
  }

  get sheetVisible(): boolean {
    return (this.state.tag === 'prompting' || this.state.tag === 'applying') && this.#canShow();
  }
  toastVisible = $derived(this.state.tag === 'confirmed');

  async check(): Promise<void> {
    try {
      const status = await this.#gw.defaults.status();
      this.#dispatch({ type: 'STATUS', status });
    } catch {
      this.#dispatch({ type: 'STATUS_FAILED' });
    }
  }

  async makeDefault(): Promise<void> {
    if (this.state.tag !== 'prompting') return;
    this.#dispatch({ type: 'MAKE_DEFAULT' });
    try {
      await this.#gw.defaults.makeDefault();
      this.#dispatch({ type: 'APPLIED' });
      if (this.#toastTimer) clearTimeout(this.#toastTimer);
      this.#toastTimer = setTimeout(() => {
        this.#toastTimer = null;
        this.#dispatch({ type: 'TOAST_DONE' });
      }, CONFIRMED_MS);
    } catch {
      this.#dispatch({ type: 'APPLY_FAILED' }); // silent
      // "Once": a failed try still counts as asked, so the sheet does not return every launch.
      try {
        await this.#gw.defaults.dismissPrompt();
      } catch {
        // best effort
      }
    }
  }

  /** Toast closed early (or its own timer fired). Safe to call twice. */
  dismissToast(): void {
    if (this.#toastTimer) clearTimeout(this.#toastTimer);
    this.#toastTimer = null;
    this.#dispatch({ type: 'TOAST_DONE' });
  }

  async notNow(): Promise<void> {
    if (this.state.tag !== 'prompting') return;
    this.#dispatch({ type: 'NOT_NOW' });
    try {
      await this.#gw.defaults.dismissPrompt();
    } catch {
      // Remembering "Not Now" is best effort.
    }
  }

  #dispatch(event: DefaultPromptEvent): void {
    this.state = defaultPromptMachine(this.state, event);
  }
}
