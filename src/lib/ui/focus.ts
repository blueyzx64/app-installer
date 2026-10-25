/** Svelte actions for keyboard focus. DOM only, no strings. */

const FOCUSABLE =
  'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

/** Focus the node (or the element matching `selector` inside it) on mount. */
export function focusOnMount(node: HTMLElement, selector?: string) {
  const target = selector ? node.querySelector<HTMLElement>(selector) : node;
  (target ?? node).focus({ preventScroll: true });
}

interface TrapOptions {
  /** Called when Escape is pressed. */
  onEscape?: () => void;
  /** Selector of the element to focus first. Defaults to the first focusable. */
  initial?: string;
}

/** Keep Tab inside `node`, move focus in on mount, call `onEscape` on Esc, restore focus on destroy. */
export function trapFocus(node: HTMLElement, options: TrapOptions = {}) {
  let opts = options;
  const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;

  const items = () => Array.from(node.querySelectorAll<HTMLElement>(FOCUSABLE));
  const first = (opts.initial ? node.querySelector<HTMLElement>(opts.initial) : null) ?? items()[0];
  (first ?? node).focus({ preventScroll: true });

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && opts.onEscape) {
      e.preventDefault();
      e.stopPropagation();
      opts.onEscape();
      return;
    }
    if (e.key !== 'Tab') return;
    const list = items();
    const head = list[0];
    const tail = list[list.length - 1];
    if (!head || !tail) {
      e.preventDefault();
      return;
    }
    const active = document.activeElement;
    if (e.shiftKey && (active === head || !node.contains(active))) {
      e.preventDefault();
      tail.focus();
    } else if (!e.shiftKey && (active === tail || !node.contains(active))) {
      e.preventDefault();
      head.focus();
    }
  }

  node.addEventListener('keydown', onKeydown);
  return {
    update(next: TrapOptions) {
      opts = next;
    },
    destroy() {
      node.removeEventListener('keydown', onKeydown);
      previous?.focus({ preventScroll: true });
    }
  };
}
