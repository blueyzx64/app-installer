<script lang="ts">
  import type { Snippet } from 'svelte';
  import { trapFocus } from './focus';

  /**
   * NSAlert. Centred 300 px card over a dim layer. Fills the nearest positioned ancestor.
   * Buttons stack full width (macOS 11+). Esc calls `oncancel`.
   */
  interface Props {
    title: string;
    message?: string;
    icon?: Snippet;
    /** Stacked buttons: default (primary) first, cancel last, as on macOS. */
    actions: Snippet;
    oncancel?: () => void;
    /** Selector to focus first. Defaults to the primary button, else the first control. */
    initialFocus?: string;
  }

  let { title, message, icon, actions, oncancel, initialFocus = '.primary' }: Props = $props();
  const uid = $props.id();
</script>

<div class="dim">
  <div
    class="alert"
    role="alertdialog"
    aria-modal="true"
    aria-labelledby="{uid}-title"
    aria-describedby={message ? `${uid}-msg` : undefined}
    use:trapFocus={{ onEscape: oncancel, initial: initialFocus }}
  >
    {#if icon}<div class="icon">{@render icon()}</div>{/if}
    <div class="text">
      <h2 id="{uid}-title">{title}</h2>
      {#if message}<p id="{uid}-msg">{message}</p>{/if}
    </div>
    <div class="actions">{@render actions()}</div>
  </div>
</div>

<style>
  .dim {
    position: absolute;
    inset: 0;
    z-index: 4;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--dim-modal);
    animation: var(--anim-fade);
  }
  .alert {
    width: var(--w-alert);
    max-width: calc(100% - 2 * var(--sp-4));
    padding: var(--sp-6) var(--sp-5) var(--sp-4h);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--sp-3h);
    text-align: center;
    background: var(--alert-bg);
    border-radius: var(--r-alert);
    box-shadow: var(--shadow-alert);
    animation: var(--anim-alert);
  }
  .icon {
    display: flex;
  }
  h2 {
    font-size: var(--fs-headline);
    line-height: 1.4;
    font-weight: 700;
  }
  p {
    margin-top: var(--sp-1h);
    font-size: var(--fs-footnote);
    line-height: var(--lh-footnote);
    color: var(--label-secondary);
  }
  .actions {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }
  .actions :global(.btn) {
    width: 100%;
  }
</style>
