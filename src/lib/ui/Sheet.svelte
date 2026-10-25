<script lang="ts">
  import type { Snippet } from 'svelte';
  import { trapFocus } from './focus';

  /**
   * NSWindow sheet. Attaches to the top of the nearest positioned ancestor (the window body),
   * slides down, and dims the content behind it. Esc calls `oncancel`.
   */
  interface Props {
    title: string;
    body?: string;
    icon?: Snippet;
    /** Buttons. Put the primary button last. */
    actions: Snippet;
    oncancel?: () => void;
  }

  let { title, body, icon, actions, oncancel }: Props = $props();
  const uid = $props.id();
</script>

<div class="dim">
  <div
    class="sheet"
    role="dialog"
    aria-modal="true"
    aria-labelledby="{uid}-title"
    aria-describedby={body ? `${uid}-body` : undefined}
    use:trapFocus={{ onEscape: oncancel, initial: '.primary' }}
  >
    {#if icon}<div class="icon">{@render icon()}</div>{/if}
    <div class="text">
      <h2 id="{uid}-title">{title}</h2>
      {#if body}<p id="{uid}-body">{body}</p>{/if}
    </div>
    <div class="actions">{@render actions()}</div>
  </div>
</div>

<style>
  .dim {
    position: absolute;
    inset: 0;
    z-index: 3;
    background: var(--dim-sheet);
    animation: var(--anim-fade);
  }
  .sheet {
    position: absolute;
    top: 0;
    left: 50%;
    width: 340px;
    max-width: 100%;
    padding: 22px 22px 18px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--sp-3);
    text-align: center;
    background: var(--surface);
    border-radius: 0 0 var(--r-sheet) var(--r-sheet);
    box-shadow: var(--shadow-sheet);
    transform: translate(-50%, 0);
    animation: var(--anim-sheet);
  }
  .icon {
    display: flex;
  }
  h2 {
    font-size: var(--fs-headline);
    line-height: var(--lh-headline);
    font-weight: 700;
  }
  p {
    margin-top: 6px;
    font-size: var(--fs-subhead);
    line-height: var(--lh-subhead);
    color: var(--label-secondary);
  }
  .actions {
    width: 100%;
    display: flex;
    gap: var(--sp-2);
    margin-top: var(--sp-1);
  }
</style>
