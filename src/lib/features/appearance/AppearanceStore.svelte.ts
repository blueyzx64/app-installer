import type { Appearance } from '$lib/core/domain/types';
import type { AppearanceGateway } from '$lib/core/ports';
import { onAccent } from './contrast';

/** Keeps the latest desktop appearance and mirrors it onto `<html>` for tokens.css. */
export class AppearanceStore {
  current = $state.raw<Appearance | null>(null);

  readonly #gw: AppearanceGateway;
  readonly #root: HTMLElement | null;
  #unsub: (() => void) | null = null;

  constructor(gateway: AppearanceGateway, root?: HTMLElement) {
    this.#gw = gateway;
    this.#root = root ?? (typeof document === 'undefined' ? null : document.documentElement);
  }

  /** Read once, then follow changes. Returns a stop function. Safe to call twice. */
  async start(): Promise<() => void> {
    if (!this.#unsub) {
      // Subscribe first so a change during get() is not lost; a late get() must not overwrite it.
      let changed = false;
      this.#unsub = this.#gw.onChange((a) => {
        changed = true;
        this.apply(a);
      });
      try {
        const first = await this.#gw.get();
        if (!changed) this.apply(first);
      } catch {
        // No appearance info: keep CSS defaults (tokens.css follows the OS query).
      }
    }
    return () => this.stop();
  }

  stop(): void {
    this.#unsub?.();
    this.#unsub = null;
  }

  apply(a: Appearance): void {
    this.current = a;
    const root = this.#root;
    if (!root) return;
    root.dataset.scheme = a.colorScheme;
    root.dataset.contrast = a.highContrast ? 'more' : 'normal';
    root.dataset.motion = a.reducedMotion ? 'reduce' : 'full';
    root.dataset.controls = a.controls.side;
    if (a.accent) {
      root.style.setProperty('--accent', a.accent);
      root.style.setProperty('--on-accent', onAccent(a.accent));
    } else {
      root.style.removeProperty('--accent');
      root.style.removeProperty('--on-accent');
    }
  }
}
