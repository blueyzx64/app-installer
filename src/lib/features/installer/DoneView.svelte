<script lang="ts">
  import { t } from '$lib/i18n';
  import Button from '$lib/ui/Button.svelte';
  import { focusOnMount } from '$lib/ui/focus';
  import CheckIcon from '$lib/ui/icons/CheckIcon.svelte';

  interface Props {
    appName: string;
    /** The app has a launcher, so Open is offered. */
    canLaunch?: boolean;
    /** Open was pressed and the app is starting. */
    launching?: boolean;
    onopen: () => void;
    ondone: () => void;
  }

  let { appName, canLaunch = true, launching = false, onopen, ondone }: Props = $props();

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      ondone();
    }
  }
</script>

<svelte:window {onkeydown} />

<div class="view">
  <div class="check"><CheckIcon /></div>
  <h2>{t('done.title', { app: appName })}</h2>
  <p>{t('done.body')}</p>
  <div class="buttons" use:focusOnMount={'.primary'}>
    <Button onclick={ondone}>{t('done.done')}</Button>
    {#if canLaunch}
      <Button variant="primary" onclick={onopen} disabled={launching}>
        {launching ? t('done.opening') : t('done.open')}
      </Button>
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
  }
  .check {
    width: 72px;
    height: 72px;
    border-radius: 50%;
    background: var(--success);
    color: var(--on-success);
    display: flex;
    align-items: center;
    justify-content: center;
    animation: var(--anim-pop);
  }
  h2 {
    margin-top: var(--sp-1);
    font-size: var(--fs-title-2);
    line-height: var(--lh-title-2);
    font-weight: 700;
    animation: var(--anim-rise);
  }
  p {
    max-width: var(--w-copy);
    line-height: 1.5;
    color: var(--label-secondary);
    animation: var(--anim-rise);
  }
  .buttons {
    display: flex;
    gap: var(--gap-row);
    margin-top: var(--gap-row);
  }
</style>
