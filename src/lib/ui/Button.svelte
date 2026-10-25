<script lang="ts">
  import type { Snippet } from 'svelte';
  import Spinner from './Spinner.svelte';

  interface Props {
    variant?: 'primary' | 'secondary' | 'destructive';
    type?: 'button' | 'submit';
    disabled?: boolean;
    /** Shows a spinner and blocks clicks while keeping the label. */
    busy?: boolean;
    /** Stretch to fill the row (sheets, alerts). */
    fill?: boolean;
    onclick?: (event: MouseEvent) => void;
    children: Snippet;
  }

  let {
    variant = 'secondary',
    type = 'button',
    disabled = false,
    busy = false,
    fill = false,
    onclick,
    children
  }: Props = $props();
</script>

<button
  class="btn {variant}"
  class:fill
  {type}
  disabled={disabled || busy}
  aria-busy={busy || undefined}
  {onclick}
>
  {#if busy}<Spinner />{/if}
  {@render children()}
</button>

<style>
  .btn {
    position: relative;
    min-height: var(--button-min);
    min-width: var(--w-button-min);
    padding: 0 var(--sp-4);
    border: 0;
    border-radius: var(--r-button);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--sp-2);
    font-size: var(--fs-body);
    font-weight: 600;
    line-height: var(--lh-body);
    cursor: pointer;
    transition: var(--tr-hover);
  }
  /* Grow the hit area to 44 px without growing the visual. */
  .btn::after {
    content: '';
    position: absolute;
    inset: calc((var(--button-min) - var(--hit-min)) / 2) 0;
  }
  .fill {
    flex: 1;
  }
  .primary {
    background: var(--accent);
    color: var(--on-accent);
  }
  .secondary {
    background: var(--fill-secondary);
    color: var(--label);
  }
  .destructive {
    background: var(--fill-secondary);
    color: var(--danger);
  }
  .btn:not(:disabled):hover {
    filter: brightness(0.95);
  }
  .btn:not(:disabled):active {
    filter: brightness(0.88);
  }
  .btn:disabled {
    cursor: default;
    opacity: 0.6;
  }
  .btn[aria-busy='true'] {
    opacity: 0.85;
  }
</style>
