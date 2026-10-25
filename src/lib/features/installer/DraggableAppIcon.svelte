<script lang="ts">
  import { t } from '$lib/i18n';
  import AppIcon from '$lib/ui/AppIcon.svelte';
  import { DragController } from './use-drag.svelte';

  interface Props {
    name: string;
    iconUrl?: string | null;
    /** Returns the drop target element (the folder tile). */
    target: () => Element | null;
    /** True once the install has started or the drop animation is running. */
    flown: boolean;
    /** Ignore input (for example while waiting for permission). */
    disabled?: boolean;
    /** True while the icon is held over the target. */
    hover?: boolean;
    ondrop: () => void;
  }

  let {
    name,
    iconUrl = null,
    target,
    flown,
    disabled = false,
    hover = $bindable(false),
    ondrop
  }: Props = $props();

  const uid = $props.id();

  const drag = new DragController({
    target: () => target(),
    disabled: () => disabled || flown,
    ondrop: () => ondrop()
  });

  // Mirror hover to the parent so the folder can react before release.
  $effect(() => {
    hover = drag.hover;
  });

  // Back to rest when the parent says the flight is over (install cancelled).
  $effect(() => {
    if (!flown && drag.flown) drag.reset();
  });
</script>

<button
  type="button"
  class="icon"
  class:dragging={drag.dragging}
  class:flown={drag.flown}
  style:transform={drag.transform}
  aria-label={t('dmg.iconLabel', { app: name })}
  aria-describedby="{uid}-kbd"
  disabled={disabled && !flown}
  onpointerdown={drag.onpointerdown}
  onpointermove={drag.onpointermove}
  onpointerup={drag.onpointerup}
  onpointercancel={drag.onpointercancel}
  onkeydown={drag.onkeydown}
>
  <AppIcon src={iconUrl} size={112} />
</button>
<span id="{uid}-kbd" class="sr-only">{t('dmg.keyboardHint')}</span>

<style>
  .icon {
    position: relative;
    z-index: 2;
    width: var(--icon-dmg);
    height: var(--icon-dmg);
    padding: 0;
    border: 0;
    border-radius: var(--r-folder);
    background: transparent;
    cursor: grab;
    touch-action: none;
    user-select: none;
    -webkit-user-select: none;
    filter: var(--icon-shadow);
    transition: filter 200ms;
    will-change: transform;
  }
  .icon.dragging {
    cursor: grabbing;
    filter: var(--icon-shadow-lifted);
    transition: filter 150ms;
  }
  .icon.flown {
    opacity: 0;
    pointer-events: none;
    transition: var(--tr-fly);
  }
</style>
