// Dev/e2e switches for the mock appearance gateway:
//   ?side=left|right  ?buttons=close,minimize,maximize  ?scheme=dark|light
//   ?motion=reduce (D-031: reduced motion only when asked)  ?textScale=1.25 (v3, default 1, 0.5 to 3)
import type { Appearance } from '../../core/domain/types';

type ControlButton = Appearance['controls']['buttons'][number];

const BUTTONS: readonly string[] = ['close', 'minimize', 'maximize'];

export function appearanceFromSearch(search: string, base: Appearance): Appearance {
  const q = new URLSearchParams(search);
  const side = q.get('side');
  const rawButtons = q.get('buttons');
  const scheme = q.get('scheme');
  let controls = base.controls;
  if (side === 'left' || side === 'right' || rawButtons !== null) {
    const picked = (rawButtons ?? '')
      .split(',')
      .map((b) => b.trim())
      .filter((b, i, all) => BUTTONS.includes(b) && all.indexOf(b) === i) as ControlButton[];
    controls = {
      side: side === 'left' || side === 'right' ? side : base.controls.side,
      buttons: picked.length > 0 ? picked : base.controls.buttons
    };
  }
  const scale = Number(q.get('textScale'));
  // textScale is a v3 field; written through a plain object so older bindings still type-check.
  const out = {
    ...base,
    colorScheme: scheme === 'dark' || scheme === 'light' ? scheme : base.colorScheme,
    reducedMotion: q.get('motion') === 'reduce' ? true : base.reducedMotion,
    textScale:
      q.has('textScale') && Number.isFinite(scale) && scale > 0
        ? Math.min(3, Math.max(0.5, scale))
        : ((base as { textScale?: number }).textScale ?? 1),
    controls
  };
  return out;
}

export function appearanceFromLocation(base: Appearance): Appearance {
  const loc = (globalThis as { location?: { search?: string } }).location;
  return loc?.search ? appearanceFromSearch(loc.search, base) : base;
}
