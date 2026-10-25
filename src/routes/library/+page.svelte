<script lang="ts">
  import { useControllers } from '$lib/app/context';
  import AppGrid from '$lib/features/library/AppGrid.svelte';
  import LibraryEmpty from '$lib/features/library/LibraryEmpty.svelte';
  import TrashConfirmAlert from '$lib/features/library/TrashConfirmAlert.svelte';
  import { t } from '$lib/i18n';
  import Button from '$lib/ui/Button.svelte';
  import Toast from '$lib/ui/Toast.svelte';
  import { NOTICE_MS } from '$lib/features/library/LibraryController.svelte';

  const { library } = useControllers();

  $effect(() => {
    void library.load();
  });

  function openPackage() {
    // TODO(v2:S4b): FilesGateway.pickPackage(), then go to the installer with the chosen path.
  }
</script>

<main data-state={library.state.tag}>
  <header>
    <h1>{t('library.title')}</h1>
    <Button onclick={openPackage}>{t('library.open')}</Button>
  </header>

  <div class="body">
    {#if library.state.tag === 'empty'}
      <LibraryEmpty onopenpackage={openPackage} />
    {:else if library.state.tag !== 'loading'}
      <AppGrid
        apps={library.apps}
        removingId={library.removingId}
        onlaunch={(id) => library.launch(id)}
        ontrash={(id) => library.requestTrash(id)}
      />
    {/if}
  </div>

  {#if library.state.tag === 'removing'}
    <p class="status" aria-live="polite">{library.removalText}</p>
  {/if}

  {#if library.confirm}
    <TrashConfirmAlert
      confirm={library.confirm}
      onconfirm={() => library.confirmTrash()}
      oncancel={() => library.cancelTrash()}
    />
  {/if}

  {#if library.notice}
    <Toast title={library.notice} duration={NOTICE_MS} ondismiss={() => library.dismissNotice()} />
  {/if}
</main>

<style>
  main {
    position: relative;
    height: 100%;
    min-width: 640px;
    min-height: 420px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  header {
    flex: none;
    padding: var(--sp-3) var(--sp-6);
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-4);
  }
  h1 {
    font-size: var(--fs-title-3);
    line-height: var(--lh-title-3);
    font-weight: 700;
  }
  .body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }
  .status {
    flex: none;
    padding: var(--sp-2) var(--sp-6);
    font-size: var(--fs-footnote);
    color: var(--label-secondary);
  }
</style>
