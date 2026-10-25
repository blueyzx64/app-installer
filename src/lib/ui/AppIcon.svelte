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
  let squircle = $state(false);
  // A new source gets a fresh chance to load.
  $effect(() => {
    void src;
    failed = false;
    squircle = false;
  });

  /**
   * DESIGN-SYSTEM section 7: mask only icons that are square AND fully opaque
   * (no own shape). Anything with transparency is shown as it is.
   */
  function isOpaqueSquare(img: HTMLImageElement): boolean {
    const w = img.naturalWidth;
    const h = img.naturalHeight;
    if (!w || w !== h) return false;
    try {
      const n = Math.min(w, 32);
      const canvas = document.createElement('canvas');
      canvas.width = n;
      canvas.height = n;
      const ctx = canvas.getContext('2d', { willReadFrequently: true });
      if (!ctx) return false;
      ctx.drawImage(img, 0, 0, n, n);
      const { data } = ctx.getImageData(0, 0, n, n);
      // Border pixels carry the shape; if all are opaque the icon has none of its own.
      for (let i = 0; i < n; i++) {
        for (const idx of [i, (n - 1) * n + i, i * n, i * n + n - 1]) {
          if ((data[idx * 4 + 3] ?? 0) < 250) return false;
        }
      }
      return true;
    } catch {
      return false; // tainted or unsupported canvas: leave the icon alone
    }
  }
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
      class:squircle
      onload={(e) => (squircle = isOpaqueSquare(e.currentTarget as HTMLImageElement))}
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
  img.squircle {
    border-radius: var(--r-squircle);
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
