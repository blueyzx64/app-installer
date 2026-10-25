<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    summary: string;
    /** Collapsed by default. */
    open?: boolean;
    children: Snippet;
  }

  let { summary, open = $bindable(false), children }: Props = $props();

  const id = $props.id();
</script>

<div class="disclosure">
  <button
    type="button"
    class="summary"
    aria-expanded={open}
    aria-controls={id}
    onclick={() => (open = !open)}
  >
    <svg class="triangle" class:open width="12" height="12" viewBox="0 0 12 12" aria-hidden="true">
      <path d="M4 2.5 L9 6 L4 9.5 Z" fill="currentColor" />
    </svg>
    <span>{summary}</span>
  </button>
  <div {id} class="content" hidden={!open}>
    {@render children()}
  </div>
</div>

<style>
  .disclosure {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }
  .summary {
    min-height: var(--hit-min);
    padding: 0;
    border: 0;
    background: none;
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
    align-self: flex-start;
    color: var(--label-secondary);
    font-size: var(--fs-footnote);
    line-height: var(--lh-body);
    cursor: pointer;
  }
  .triangle {
    flex: none;
    transition: var(--tr-disclosure);
  }
  .triangle.open {
    transform: rotate(90deg);
  }
  .content[hidden] {
    display: none;
  }
</style>
