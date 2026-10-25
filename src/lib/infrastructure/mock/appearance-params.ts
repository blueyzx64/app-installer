// Dev/e2e switches for the mock appearance gateway:
//   ?side=left|right  ?buttons=close,minimize,maximize  ?scheme=dark|light
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
  return {
    ...base,
    colorScheme: scheme === 'dark' || scheme === 'light' ? scheme : base.colorScheme,
    controls
  };
}

export function appearanceFromLocation(base: Appearance): Appearance {
  const loc = (globalThis as { location?: { search?: string } }).location;
  return loc?.search ? appearanceFromSearch(loc.search, base) : base;
}
