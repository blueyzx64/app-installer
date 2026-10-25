<script lang="ts">
  import type { InstalledApp } from '$lib/core/domain/types';
  import { t } from '$lib/i18n';
  import AppIcon from '$lib/ui/AppIcon.svelte';
  import { duration, isReducedMotion } from '$lib/ui/motion';
  import { fade, scale } from 'svelte/transition';

  interface Props {
    app: InstalledApp;
    /** Roving tabindex: only one tile in the grid is a tab stop. */
    focusable?: boolean;
    /** Removal in progress: dimmed, not clickable. */
    busy?: boolean;
    menuOpen?: boolean;
    /** Label of the trash action: "Move to Trash" (user app) or "Remove" (system app). */
    trashLabel: string;
    onlaunch: (id: string) => void;
    ontrash: (id: string) => void;
    onmenu: (id: string | null) => void;
  }

  let {
    app,
    focusable = false,
    busy = false,
    menuOpen = false,
    trashLabel,
    onlaunch,
    ontrash,
    onmenu
  }: Props = $props();

  let tile = $state<HTMLButtonElement | null>(null);
  let menuEl = $state<HTMLDivElement | null>(null);

  /** Leaves with a shrink and fade; with reduced motion, a short fade only. */
  function leave(node: Element) {
    return isReducedMotion()
      ? fade(node, { duration: duration.reduced })
      : scale(node, { duration: duration.gulp, start: 0.8 });
  }

  function open() {
    if (app.canLaunch && !busy) onlaunch(app.id);
  }

  function onkeydown(e: KeyboardEvent) {
    if (busy) return;
    if (e.key === 'Delete' || e.key === 'Backspace') {
      e.preventDefault();
      ontrash(app.id);
    } else if (e.key === 'ContextMenu' || (e.key === 'F10' && e.shiftKey)) {
      e.preventDefault();
      onmenu(app.id);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      open();
    }
  }

  function oncontextmenu(e: MouseEvent) {
    e.preventDefault();
    if (!busy) onmenu(app.id);
  }

  // Close on outside press; focus the first item when opened.
  $effect(() => {
    if (!menuOpen) return;
    menuEl?.querySelector<HTMLElement>('[role="menuitem"]')?.focus();
    const onpress = (e: Event) => {
      if (!menuEl?.contains(e.target as Node)) onmenu(null);
    };
    window.addEventListener('pointerdown', onpress);
    return () => window.removeEventListener('pointerdown', onpress);
  });

  function onmenukey(e: KeyboardEvent) {
    const items = [...(menuEl?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? [])];
    const i = items.indexOf(document.activeElement as HTMLElement);
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      onmenu(null);
      tile?.focus();
    } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      e.stopPropagation();
      const d = e.key === 'ArrowDown' ? 1 : -1;
      items[(i + d + items.length) % items.length]?.focus();
    }
  }
</script>

<div class="cell" out:leave|global data-app-id={app.id}>
  <button
    bind:this={tile}
    class="tile"
    class:busy
    type="button"
    data-tile
    tabindex={focusable ? 0 : -1}
    aria-haspopup="menu"
    aria-expanded={menuOpen}
    aria-busy={busy || undefined}
    disabled={busy}
    ondblclick={open}
    {onkeydown}
    {oncontextmenu}
  >
    <AppIcon src={app.iconDataUrl} size={72} />
    <span class="name">{app.displayName}</span>
  </button>

  <button
    class="more"
    type="button"
    aria-haspopup="menu"
    aria-expanded={menuOpen}
    aria-label={t('library.more', { app: app.displayName })}
    disabled={busy}
    tabindex={focusable ? 0 : -1}
    onpointerdown={(e) => e.stopPropagation()}
    onclick={() => onmenu(menuOpen ? null : app.id)}
  >
    <span aria-hidden="true">⋯</span>
  </button>

  {#if menuOpen}
    <div
      class="menu"
      role="menu"
      tabindex="-1"
      aria-label={t('library.menu', { app: app.displayName })}
      bind:this={menuEl}
      onkeydown={onmenukey}
    >
      {#if app.canLaunch}
        <button
          type="button"
          role="menuitem"
          onclick={() => {
            onmenu(null);
            onlaunch(app.id);
          }}>{t('library.launch')}</button
        >
      {/if}
      <button
        type="button"
        role="menuitem"
        onclick={() => {
          onmenu(null);
          ontrash(app.id);
        }}>{trashLabel}</button
      >
    </div>
  {/if}
</div>

<style>
  .cell {
    position: relative;
    width: 112px;
  }
  .tile {
    width: 100%;
    min-height: 44px;
    padding: var(--sp-2) var(--sp-1);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--sp-2);
    border-radius: var(--r-button);
    background: transparent;
    color: var(--label);
    cursor: default;
  }
  .tile:hover {
    background: var(--fill-hover);
  }
  .tile:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .tile.busy {
    opacity: 0.5;
  }
  .name {
    max-width: 100%;
    font-size: var(--fs-footnote);
    line-height: var(--lh-footnote);
    text-align: center;
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow-wrap: anywhere;
  }
  .more {
    position: absolute;
    top: 0;
    right: 0;
    width: 28px;
    height: 28px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--fill-secondary);
    color: var(--label-secondary);
    line-height: 1;
  }
  /* The visible circle is small; the touch target is 44 px. */
  .more::after {
    content: '';
    position: absolute;
    inset: -8px;
  }
  .more:hover,
  .more:focus-visible {
    background: var(--accent);
    color: var(--on-accent);
    outline: none;
  }
  .menu {
    position: absolute;
    top: 80px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 3;
    min-width: 160px;
    padding: var(--sp-1);
    display: flex;
    flex-direction: column;
    border-radius: var(--r-sheet);
    background: var(--material-toast);
    -webkit-backdrop-filter: var(--material-blur);
    backdrop-filter: var(--material-blur);
    box-shadow: var(--shadow-toast);
    animation: var(--anim-fade);
  }
  .menu button {
    min-height: 44px;
    padding: 0 var(--sp-3);
    border-radius: var(--r-button);
    text-align: left;
    color: var(--label);
    font-size: var(--fs-body);
    background: transparent;
  }
  .menu button:hover,
  .menu button:focus-visible {
    background: var(--accent);
    color: var(--on-accent);
    outline: none;
  }
</style>
