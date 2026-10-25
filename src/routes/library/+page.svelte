<script lang="ts">
  // Placeholder markup. S6 replaces it with AppGrid / AppTile / TrashConfirm.
  import { useControllers } from '$lib/app/context';
  import { t } from '$lib/i18n';

  const { library } = useControllers();

  $effect(() => {
    void library.load();
  });
</script>

<main data-state={library.state.tag}>
  <h1>{t('library.title')}</h1>

  {#if library.state.tag === 'empty'}
    <p>{t('library.empty')}</p>
  {/if}

  <ul>
    {#each library.apps as app (app.id)}
      <li>
        {app.displayName}
        <button onclick={() => library.launch(app.id)}>{t('library.launch')}</button>
        <button onclick={() => library.requestTrash(app.id)}>{t('trash.confirm.system')}</button>
      </li>
    {/each}
  </ul>

  {#if library.confirm}
    <div role="alertdialog" aria-label={library.confirm.title}>
      <p>{library.confirm.title}</p>
      <p>{library.confirm.body}</p>
      <button onclick={() => library.cancelTrash()}>{library.confirm.cancelLabel}</button>
      <button onclick={() => library.confirmTrash()}>{library.confirm.confirmLabel}</button>
    </div>
  {/if}

  {#if library.state.tag === 'removing'}<p aria-live="polite">{library.removalText}</p>{/if}
  {#if library.notice}<p role="status">{library.notice}</p>{/if}
</main>
