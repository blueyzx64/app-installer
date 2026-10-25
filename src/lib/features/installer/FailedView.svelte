<script lang="ts">
  import { t } from '$lib/i18n';
  import AppIcon from '$lib/ui/AppIcon.svelte';
  import Button from '$lib/ui/Button.svelte';
  import { focusOnMount } from '$lib/ui/focus';

  /** Install failed. Message is plain language, already resolved by the controller. */
  interface Props {
    appName: string;
    iconUrl?: string | null;
    message: string;
    /** Offer Try Again. */
    canRetry?: boolean;
    onclose: () => void;
    onretry?: () => void;
  }

  let { appName, iconUrl = null, message, canRetry = false, onclose, onretry }: Props = $props();

  const offerRetry = $derived(canRetry && !!onretry);

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      onclose();
    }
  }
</script>

<svelte:window {onkeydown} />

<div class="view" data-app={appName}>
  <AppIcon src={iconUrl} size={72} />
  <p class="message" role="alert">{message}</p>
  <div class="buttons" use:focusOnMount={offerRetry ? '.primary' : 'button'}>
    <Button variant={offerRetry ? 'secondary' : 'primary'} onclick={onclose}>
      {t('action.close')}
    </Button>
    {#if offerRetry}
      <Button variant="primary" onclick={onretry}>{t('action.tryAgain')}</Button>
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
  .message {
    max-width: 380px;
    font-size: var(--fs-title-3);
    line-height: var(--lh-title-3);
    font-weight: 700;
  }
  .buttons {
    display: flex;
    gap: 10px;
    margin-top: 10px;
  }
</style>
