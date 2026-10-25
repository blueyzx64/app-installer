<script lang="ts">
  /** 6 px progress bar. `value` is 0 to 100; `null` means indeterminate (sliding segment). */
  interface Props {
    value: number | null;
    /** Accessible name, already translated. */
    label: string;
  }
  let { value, label }: Props = $props();

  const pct = $derived(value === null ? null : Math.max(0, Math.min(100, Math.round(value))));
</script>

<div
  class="track"
  role="progressbar"
  aria-label={label}
  aria-valuemin="0"
  aria-valuemax="100"
  aria-valuenow={pct ?? undefined}
>
  {#if pct === null}
    <div class="fill indeterminate"></div>
  {:else}
    <div class="fill" style:width="{pct}%"></div>
  {/if}
</div>

<style>
  .track {
    height: 6px;
    border-radius: var(--r-track);
    background: var(--track);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    border-radius: var(--r-track);
    background: var(--accent);
    transition: var(--tr-progress);
  }
  .indeterminate {
    width: 30%;
    animation: var(--anim-indeterminate);
  }
  /* Reduced motion swaps the slide for a pulse, so the segment must fill the track. */
  :global(:root[data-motion='reduce']) .indeterminate {
    width: 100%;
  }
  @media (prefers-reduced-motion: reduce) {
    :global(:root:not([data-motion='full'])) .indeterminate {
      width: 100%;
    }
  }
</style>
