<script lang="ts">
  import type { Snippet } from 'svelte';
  import { onMount } from 'svelte';
  import type { Appearance } from '$lib/core/domain/types';
  import type { WindowGateway } from '$lib/core/ports';
  import WindowControls from './WindowControls.svelte';

  /**
   * Our own title bar (the window has no system decorations). The bar is the drag region;
   * the controls sit on the side GNOME's button-layout says (ADR-0006). The title stays centred.
   */
  interface Props {
    title: string;
    controls?: Controls;
    window: WindowGateway;
    children: Snippet;
  }

  type Controls = Appearance['controls'];
  const DEFAULT_CONTROLS: Controls = { side: 'right', buttons: ['close'] };

  let { title, controls = DEFAULT_CONTROLS, window: win, children }: Props = $props();

  let focused = $state(typeof document === 'undefined' ? true : document.hasFocus());

  onMount(() => {
    const on = () => (focused = true);
    const off = () => (focused = false);
    focused = document.hasFocus();
    window.addEventListener('focus', on);
    window.addEventListener('blur', off);
    return () => {
      window.removeEventListener('focus', on);
      window.removeEventListener('blur', off);
    };
  });

  // Fallback for webviews where data-tauri-drag-region does not start a drag.
  function onpointerdown(e: PointerEvent) {
    if (e.button !== 0 || e.target !== e.currentTarget) return;
    void win.startDragging().catch(() => {});
  }
</script>

<div class="chrome" data-window-chrome>
  <header
    class="bar"
    role="presentation"
    data-tauri-drag-region
    data-side={controls.side}
    {onpointerdown}
  >
    <WindowControls
      {controls}
      {focused}
      onclose={() => void win.close().catch(() => {})}
      onminimize={() => void win.minimize().catch(() => {})}
    />
    <h1 class="title" class:dim={!focused}>{title}</h1>
  </header>
  <div class="body">
    {@render children()}
  </div>
</div>

<style>
  .chrome {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
    border-radius: var(--r-window);
    background: var(--window-bg);
    color: var(--label);
  }
  .bar {
    position: relative;
    flex: none;
    height: 40px;
    display: flex;
    align-items: center;
    background: var(--titlebar-bg);
    border-bottom: var(--hairline) solid var(--separator);
    user-select: none;
  }
  /* Controls on the right: push them there. The title is centred independent of side. */
  .bar[data-side='right'] {
    justify-content: flex-end;
  }
  .title {
    position: absolute;
    inset: 0;
    margin: 0;
    display: grid;
    place-items: center;
    font-size: var(--fs-headline);
    font-weight: 600;
    color: var(--label);
    pointer-events: none;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .title.dim {
    color: var(--label-secondary);
  }
  .body {
    flex: 1;
    min-height: 0;
    position: relative;
  }
</style>
