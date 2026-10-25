<script lang="ts">
  // Placeholder markup. S6 replaces it with the real installer views (features/installer/*.svelte).
  import { useControllers } from '$lib/app/context';
  import { t } from '$lib/i18n';

  const { installer } = useControllers();

  $effect(() => installer.start());
</script>

<main data-state={installer.state.tag}>
  {#if installer.preview}
    <h1>{t('dmg.hint', { app: installer.appName })}</h1>
  {/if}

  {#if installer.state.tag === 'ready'}
    <button onclick={() => installer.keyInstall()}>{t('dmg.folder')}</button>
  {:else if installer.busy}
    <p aria-live="polite">{installer.phaseText}</p>
    {#if installer.percent !== null}<progress max="100" value={installer.percent}></progress>{/if}
    <p>{installer.etaText}</p>
    {#if installer.canCancel}
      <button onclick={() => installer.cancelInstall()}>{t('trash.cancel')}</button>
    {/if}
  {:else if installer.state.tag === 'succeeded'}
    <p>{t('done.title', { app: installer.appName })}</p>
    <button onclick={() => installer.openApp()}>{t('done.open')}</button>
    <button onclick={() => installer.close()}>{t('done.done')}</button>
  {:else if installer.incompatible}
    <p>{installer.incompatible.title}</p>
    {#if installer.incompatible.body}<p>{installer.incompatible.body}</p>{/if}
    {#if installer.incompatible.action === 'open'}
      <button onclick={() => installer.openInstalled()}>{t('action.open')}</button>
    {/if}
    <button onclick={() => installer.close()}>{t('action.close')}</button>
  {:else if installer.error}
    <p>{installer.error.message}</p>
    {#if installer.error.action === 'tryAgain'}
      <button onclick={() => installer.retry()}>{t('action.tryAgain')}</button>
    {/if}
    <button onclick={() => installer.close()}>{t('action.close')}</button>
  {/if}
</main>
