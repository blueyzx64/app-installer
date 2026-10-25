<script lang="ts">
  import { text, trashCopy } from '$lib/core/copy/copy';
  import type { InstalledApp } from '$lib/core/domain/types';
  import { t } from '$lib/i18n';
  import { duration, isReducedMotion } from '$lib/ui/motion';
  import { flip } from 'svelte/animate';
  import AppTile from './AppTile.svelte';

  interface Props {
    apps: InstalledApp[];
    /** App being removed right now. Its tile is dimmed. */
    removingId?: string | null;
    onlaunch: (id: string) => void;
    ontrash: (id: string) => void;
  }

  let { apps, removingId = null, onlaunch, ontrash }: Props = $props();

  let grid = $state<HTMLUListElement | null>(null);
  let activeId = $state<string | null>(null);
  let menuId = $state<string | null>(null);

  const current = $derived(
    activeId && apps.some((a) => a.id === activeId) ? activeId : (apps[0]?.id ?? null)
  );

  function tiles(): HTMLElement[] {
    return [...(grid?.querySelectorAll<HTMLElement>('[data-tile]') ?? [])];
  }

  /** Tiles per row, read from the layout (tiles on the first row share one offsetTop). */
  function columns(list: HTMLElement[]): number {
    if (list.length === 0) return 1;
    const top = list[0]!.offsetTop;
    const n = list.findIndex((el) => el.offsetTop !== top);
    return n === -1 ? list.length : n;
  }

  function focusIndex(i: number) {
    const list = tiles();
    const el = list[Math.max(0, Math.min(list.length - 1, i))];
    if (!el) return;
    activeId = el.closest<HTMLElement>('[data-app-id]')?.dataset.appId ?? null;
    el.focus();
  }

  function onkeydown(e: KeyboardEvent) {
    if (menuId !== null) return;
    const list = tiles();
    const i = list.indexOf(document.activeElement as HTMLElement);
    if (i === -1) return;
    const cols = columns(list);
    const target: Record<string, number> = {
      ArrowRight: i + 1,
      ArrowLeft: i - 1,
      ArrowDown: i + cols,
      ArrowUp: i - cols,
      Home: 0,
      End: list.length - 1
    };
    const next = target[e.key];
    if (next === undefined) return;
    e.preventDefault();
    // Down on the last row stays put rather than jumping to the end.
    if (next >= 0 && next < list.length) focusIndex(next);
  }

  function flipDuration() {
    return isReducedMotion() ? 0 : duration.rise;
  }
</script>

<!-- Arrow keys bubble up from the tiles, which hold the focus. -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<ul
  class="grid"
  bind:this={grid}
  aria-label={t('library.grid')}
  {onkeydown}
  onfocusin={(e) => {
    const id = (e.target as HTMLElement).closest<HTMLElement>('[data-app-id]')?.dataset.appId;
    if (id) activeId = id;
  }}
>
  {#each apps as app (app.id)}
    <li animate:flip={{ duration: flipDuration() }}>
      <AppTile
        {app}
        focusable={app.id === current}
        busy={app.id === removingId}
        menuOpen={menuId === app.id}
        trashLabel={text(trashCopy(app).confirm)}
        {onlaunch}
        {ontrash}
        onmenu={(id) => (menuId = id)}
      />
    </li>
  {/each}
</ul>

<style>
  .grid {
    list-style: none;
    margin: 0;
    padding: var(--sp-6);
    display: grid;
    grid-template-columns: repeat(auto-fill, 112px);
    gap: var(--sp-4);
    justify-content: start;
    align-content: start;
  }
</style>
