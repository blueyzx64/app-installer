<script lang="ts">
  import type { WindowControls } from '$lib/core/domain/types';
  import { t } from '$lib/i18n';

  /**
   * macOS-style circles. Which ones, and in what order, come from GNOME's button-layout
   * (ADR-0006). The side only decides where the parent puts this group (`data-side`).
   * Zoom is always disabled: the window is not resizable.
   */
  interface Props {
    controls: WindowControls;
    /** Unfocused windows show grey circles. */
    focused?: boolean;
    onclose?: () => void;
    onminimize?: () => void;
  }

  let { controls, focused = true, onclose, onminimize }: Props = $props();
</script>

<div class="controls" class:unfocused={!focused} data-side={controls.side} role="group">
  {#each controls.buttons as kind (kind)}
    {#if kind === 'close'}
      <button
        type="button"
        class="dot close"
        aria-label={t('window.close')}
        onclick={() => onclose?.()}
      >
        <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M3.5 3.5l5 5m0-5l-5 5" /></svg>
      </button>
    {:else if kind === 'minimize'}
      <button
        type="button"
        class="dot min"
        aria-label={t('window.minimize')}
        onclick={() => onminimize?.()}
      >
        <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M3 6h6" /></svg>
      </button>
    {:else}
      <!-- Disabled and decorative: it cannot be used, so it is hidden from assistive tech. -->
      <button type="button" class="dot zoom" disabled aria-hidden="true" tabindex="-1"></button>
    {/if}
  {/each}
</div>

<style>
  .controls {
    --glyph: rgb(0 0 0 / 55%);
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    padding-inline: var(--sp-3);
  }
  .dot {
    position: relative;
    width: 14px;
    height: 14px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    display: grid;
    place-items: center;
    color: var(--glyph);
    cursor: pointer;
    transition: var(--tr-hover);
  }
  /* Bigger hit area than the visible circle. */
  .dot::after {
    content: '';
    position: absolute;
    inset: -8px -4px;
  }
  .close {
    background: var(--light-close);
  }
  .min {
    background: var(--light-min);
  }
  .zoom {
    background: var(--light-zoom);
    cursor: default;
  }
  svg {
    width: 10px;
    height: 10px;
    fill: none;
    stroke: currentcolor;
    stroke-width: 1.4;
    stroke-linecap: round;
    opacity: 0;
  }
  /* Glyphs show for the whole group on hover or keyboard focus, as on macOS. */
  .controls:hover svg,
  .controls:focus-within svg {
    opacity: 1;
  }
  .dot:not(:disabled):active {
    filter: brightness(0.85);
  }
  .dot:focus-visible {
    outline: var(--focus-width) solid var(--accent);
    outline-offset: var(--focus-offset);
  }
  /* Window not focused: all circles grey until the pointer is over the group. */
  .unfocused:not(:hover) .dot {
    background: var(--light-inactive);
  }
  .zoom:disabled {
    opacity: 1;
  }
</style>
