<script lang="ts">
  import type { Snippet } from 'svelte';
  import AppGlyph from './icons/AppGlyph.svelte';

  interface Props {
    /** data: URL or other image source. null or missing shows the generic app glyph. */
    src?: string | null;
    size?: number;
    /** Accessible name. Omit for decorative use (a visible label sits next to it). */
    label?: string;
    /** Optional badge drawn bottom-right (for example a lock). */
    badge?: Snippet;
  }

  let { src = null, size = 112, label, badge }: Props = $props();

  let failed = $state(false);
  // A new source gets a fresh chance to load.
  $effect(() => {
    void src;
    failed = false;
  });
</script>

<span
  class="icon"
  style:width="{size}px"
  style:height="{size}px"
  role={label ? 'img' : undefined}
  aria-label={label}
  aria-hidden={label ? undefined : 'true'}
>
  {#if src && !failed}
    <img
      {src}
      alt=""
      width={size}
      height={size}
      draggable="false"
      onerror={() => (failed = true)}
    />
  {:else}
    <AppGlyph {size} />
  {/if}
  {#if badge}<span class="badge">{@render badge()}</span>{/if}
</span>

<style>
  .icon {
    position: relative;
    display: inline-flex;
    flex: none;
  }
  img {
    display: block;
    object-fit: contain;
    -webkit-user-drag: none;
  }
  .badge {
    position: absolute;
    right: -6px;
    bottom: -6px;
    width: 28px;
    height: 28px;
    border-radius: 50%;
    background: var(--surface);
    color: var(--label-tertiary);
    box-shadow: var(--shadow-badge);
    display: flex;
    align-items: center;
    justify-content: center;
  }
</style>
