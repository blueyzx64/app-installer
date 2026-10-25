<script lang="ts">
  import type { Snippet } from 'svelte';
  import { duration } from './motion';

  /**
   * NSUserNotification banner. Top-right inside the window, 300 px, translucent material.
   * Calls `ondismiss` after `duration` ms. Strings come in as props (i18n happens upstream).
   */
  interface Props {
    title: string;
    body?: string;
    icon?: Snippet;
    /** Milliseconds before auto-dismiss. */
    duration?: number;
    ondismiss: () => void;
  }

  let {
    title,
    body,
    icon,
    duration: visibleFor = duration.toastVisible,
    ondismiss
  }: Props = $props();

  $effect(() => {
    const timer = setTimeout(ondismiss, visibleFor);
    return () => clearTimeout(timer);
  });
</script>

<div class="toast" role="status" aria-live="polite">
  {#if icon}<div class="icon" aria-hidden="true">{@render icon()}</div>{/if}
  <div class="text">
    <p class="title">{title}</p>
    {#if body}<p class="body">{body}</p>{/if}
  </div>
</div>

<style>
  .toast {
    position: absolute;
    top: 36px;
    right: 14px;
    z-index: 5;
    width: 300px;
    max-width: calc(100% - 28px);
    padding: 12px 14px;
    display: flex;
    align-items: flex-start;
    gap: var(--sp-3);
    border-radius: 14px;
    background: var(--material-toast);
    -webkit-backdrop-filter: var(--material-blur);
    backdrop-filter: var(--material-blur);
    box-shadow: var(--shadow-toast);
    animation: var(--anim-toast);
  }
  .icon {
    display: flex;
    flex-shrink: 0;
  }
  .text {
    min-width: 0;
  }
  .title {
    font-size: var(--fs-headline);
    line-height: var(--lh-headline);
    font-weight: 700;
  }
  .body {
    margin-top: 2px;
    font-size: var(--fs-footnote);
    line-height: var(--lh-footnote);
    color: var(--label-secondary);
  }
</style>
