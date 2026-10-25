import { Spring } from 'svelte/motion';
import { isReducedMotion, spring } from '$lib/ui/motion';

export interface DragOptions {
  /** The drop target (the Applications folder tile). Read on every move. */
  target: () => Element | null;
  /** True while dragging must be ignored (already dropped, waiting, etc). */
  disabled: () => boolean;
  /** Called once when the icon is released over the target, or activated by keyboard. */
  ondrop: () => void;
}

const FLY_SCALE = 0.25;

/**
 * Pointer drag for the app icon. Pointer events only (mouse, touch, pen); no HTML5 DnD.
 *
 * The icon follows the pointer one-to-one. Releasing off target springs it back
 * (instant when motion is reduced). Releasing on target "flies" it into the folder.
 * Enter or Space on the focused icon is the keyboard path to the same drop.
 */
export class DragController {
  dragging = $state(false);
  hover = $state(false);
  /** True from the moment of drop until `reset()`. */
  flown = $state(false);

  #pos = new Spring({ x: 0, y: 0 }, spring.drag);
  #fly = $state({ x: 0, y: 0, s: 1 });
  #origin = { x: 0, y: 0 };
  #scale = 1;
  #options: DragOptions;

  constructor(options: DragOptions) {
    this.#options = options;
  }

  /** Inline `transform` for the icon. */
  transform = $derived.by(() => {
    if (this.flown) {
      return `translate(${this.#fly.x}px, ${this.#fly.y}px) scale(${this.#fly.s})`;
    }
    const { x, y } = this.#pos.current;
    return this.dragging
      ? `translate(${x}px, ${y}px) scale(var(--lift-scale))`
      : `translate(${x}px, ${y}px)`;
  });

  onpointerdown = (e: PointerEvent) => {
    if (this.#options.disabled() || this.flown || (e.button !== undefined && e.button > 0)) return;
    const el = e.currentTarget as HTMLElement;
    const rect = el.getBoundingClientRect();
    this.#scale = rect.width && el.offsetWidth ? rect.width / el.offsetWidth : 1;
    this.#origin = { x: e.clientX, y: e.clientY };
    try {
      el.setPointerCapture(e.pointerId);
    } catch {
      /* not supported in every environment; the drag still works inside the element */
    }
    this.dragging = true;
    this.hover = false;
    void this.#pos.set({ x: 0, y: 0 }, { instant: true });
  };

  onpointermove = (e: PointerEvent) => {
    if (!this.dragging) return;
    const r = this.#options.target()?.getBoundingClientRect();
    this.hover =
      !!r &&
      e.clientX >= r.left &&
      e.clientX <= r.right &&
      e.clientY >= r.top &&
      e.clientY <= r.bottom;
    void this.#pos.set(
      {
        x: (e.clientX - this.#origin.x) / this.#scale,
        y: (e.clientY - this.#origin.y) / this.#scale
      },
      { instant: true }
    );
  };

  onpointerup = (e: PointerEvent) => {
    if (!this.dragging) return;
    const el = e.currentTarget as HTMLElement;
    try {
      el.releasePointerCapture(e.pointerId);
    } catch {
      /* ignore */
    }
    if (this.hover) {
      this.#drop(el);
    } else {
      this.#cancel();
    }
  };

  onpointercancel = () => {
    if (this.dragging) this.#cancel();
  };

  onkeydown = (e: KeyboardEvent) => {
    if (e.key !== 'Enter' && e.key !== ' ') return;
    e.preventDefault();
    if (this.#options.disabled() || this.flown) return;
    this.#drop(e.currentTarget as HTMLElement);
  };

  /** Put the icon back at rest (after a cancelled install). */
  reset() {
    this.dragging = false;
    this.hover = false;
    this.flown = false;
    void this.#pos.set({ x: 0, y: 0 }, { instant: true });
  }

  #cancel() {
    this.dragging = false;
    this.hover = false;
    void this.#pos.set({ x: 0, y: 0 }, { instant: isReducedMotion() });
  }

  #drop(el: HTMLElement) {
    // Fly from where the icon rests to the centre of the folder.
    const rect = el.getBoundingClientRect();
    const t = this.#options.target()?.getBoundingClientRect();
    const { x, y } = this.#pos.current;
    if (t && !isReducedMotion()) {
      const restX = rect.left + rect.width / 2 - x * this.#scale;
      const restY = rect.top + rect.height / 2 - y * this.#scale;
      this.#fly = {
        x: (t.left + t.width / 2 - restX) / this.#scale,
        y: (t.top + t.height / 2 - restY) / this.#scale,
        s: FLY_SCALE
      };
    } else {
      // Reduced motion: no flight, the icon fades where it is.
      this.#fly = { x, y, s: 1 };
    }
    this.dragging = false;
    this.hover = false;
    this.flown = true;
    this.#options.ondrop();
  }
}
