/**
 * Motion presets. CSS owns the visuals (tokens.css); these constants are for
 * JS that must wait for or drive an animation. Keep the numbers in sync with
 * DESIGN-SYSTEM section 4.
 */

/** Svelte `Spring` options for the icon that follows the pointer. */
export const spring = {
  drag: { stiffness: 0.2, damping: 0.65 }
} as const;

export const duration = {
  sheet: 260,
  alert: 260,
  pop: 560,
  fly: 500,
  gulp: 420,
  gulpDelay: 320,
  rise: 340,
  toast: 340,
  /** Cross-fade used everywhere when motion is reduced. */
  reduced: 120,
  /** How long a dropped icon takes to reach the folder before we continue. */
  flight: 560,
  toastVisible: 4000
} as const;

export const ease = {
  sheet: 'cubic-bezier(.2,.9,.3,1)',
  alert: 'cubic-bezier(.2,.9,.3,1.1)',
  pop: 'cubic-bezier(.2,.9,.3,1.2)',
  fly: 'cubic-bezier(.4,0,.2,1)'
} as const;

/**
 * True when the user wants reduced motion. Mirrors the CSS in tokens.css:
 * `data-motion="reduce"` wins, `data-motion="full"` wins, otherwise the OS query.
 */
export function isReducedMotion(): boolean {
  if (typeof document === 'undefined') return false;
  const mode = document.documentElement.dataset.motion;
  if (mode === 'reduce') return true;
  if (mode === 'full') return false;
  return typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;
}

/** Milliseconds to wait for an animation of `full` ms, honouring reduced motion. */
export function motionMs(full: number): number {
  return isReducedMotion() ? Math.min(full, duration.reduced) : full;
}
