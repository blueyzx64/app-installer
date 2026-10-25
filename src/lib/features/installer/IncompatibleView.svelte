<script lang="ts">
  import { t } from '$lib/i18n';
  import AppIcon from '$lib/ui/AppIcon.svelte';
  import Button from '$lib/ui/Button.svelte';
  import { focusOnMount } from '$lib/ui/focus';

  /** "Can't install this" and "already installed" screen. Renders the controller's text. */
  interface Props {
    title: string;
    body?: string | null;
    iconUrl?: string | null;
    /** Offer Open (controller: action === 'open'). */
    canOpen?: boolean;
    onclose: () => void;
    onopen?: () => void;
  }

  let { title, body = null, iconUrl = null, canOpen = false, onclose, onopen }: Props = $props();

  const offerOpen = $derived(canOpen && !!onopen);

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      onclose();
    }
  }
</script>

<svelte:window {onkeydown} />

<div class="view">
  <AppIcon src={iconUrl} size={72} />
  <h2>{title}</h2>
  {#if body}<p>{body}</p>{/if}
  <div class="buttons" use:focusOnMount={offerOpen ? '.primary' : 'button'}>
    <Button variant={offerOpen ? 'secondary' : 'primary'} onclick={onclose}>
      {t('action.close')}
    </Button>
    {#if offerOpen}
      <Button variant="primary" onclick={onopen}>{t('done.open')}</Button>
    {/if}
  </div>
</div>

<style>
  .view {
    height: 100%;
    padding: var(--sp-8);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--sp-3);
    text-align: center;
    animation: var(--anim-rise);
  }
  h2 {
    margin-top: var(--sp-1);
    max-width: var(--w-copy-wide);
    font-size: var(--fs-title-3);
    line-height: var(--lh-title-3);
    font-weight: 700;
  }
  p {
    max-width: var(--w-copy);
    line-height: 1.5;
    color: var(--label-secondary);
  }
  .buttons {
    display: flex;
    gap: var(--gap-row);
    margin-top: var(--gap-row);
  }
</style>
