<script lang="ts">
  // Install window: drives S6's views from InstallerController. No logic here beyond wiring.
  import { goto } from '$app/navigation';
  import { resolve } from '$app/paths';
  import { useControllers } from '$lib/app/context';
  import { t } from '$lib/i18n';
  import DmgView from '$lib/features/installer/DmgView.svelte';
  import DoneView from '$lib/features/installer/DoneView.svelte';
  import IncompatibleView from '$lib/features/installer/IncompatibleView.svelte';
  import ProgressView from '$lib/features/installer/ProgressView.svelte';
  import Alert from '$lib/ui/Alert.svelte';
  import AppIcon from '$lib/ui/AppIcon.svelte';
  import Button from '$lib/ui/Button.svelte';
  import Disclosure from '$lib/ui/Disclosure.svelte';
  import Sheet from '$lib/ui/Sheet.svelte';
  import Spinner from '$lib/ui/Spinner.svelte';
  import Toast from '$lib/ui/Toast.svelte';

  const { installer, defaultHandler } = useControllers();

  let root = $state<HTMLElement | null>(null);

  // F-21: the app icon is the first Tab stop on the ready screen (the close button stays reachable).
  // Not while the default-handler sheet is up: it owns focus and hands it back when it closes.
  $effect(() => {
    if (installer.state.tag === 'ready' && !defaultHandler.sheetVisible) {
      root?.querySelector<HTMLElement>('button.icon')?.focus({ preventScroll: true });
    }
  });

  // Escape on the error screen = its Close button (the Alert's cancel). Close ends this
  // package; the window closes only when no other package is waiting. Busy screens ignore Esc.
  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && !e.defaultPrevented && installer.state.tag === 'failed') {
      e.preventDefault();
      installer.close();
    }
  }

  // Nothing opened (direct visit, or the window stayed open after Close): show the library.
  $effect(() => {
    if (installer.state.tag === 'idle') {
      try {
        goto(resolve('/library'), { replaceState: true }).catch(() => {});
      } catch {
        /* no router (component tests) */
      }
    }
  });
</script>

<svelte:window {onkeydown} />

<main class="install" data-state={installer.state.tag} bind:this={root}>
  {#if installer.showDmg}
    <DmgView
      appName={installer.appName}
      iconUrl={installer.iconUrl}
      needsPassword={installer.needsPassword}
      isUpdate={installer.isUpdate}
      started={installer.state.tag === 'starting' || installer.state.tag === 'awaitingPermission'}
      waitingForPermission={installer.waitingForPermission}
      oninstall={() => installer.keyInstall()}
      oncancel={() => installer.cancelInstall()}
    />
  {:else if installer.state.tag === 'installing'}
    <ProgressView
      appName={installer.appName}
      iconUrl={installer.iconUrl}
      percent={installer.percent}
      canCancel={installer.allowCancel}
      oncancel={() => installer.cancel()}
      phaseText={installer.phaseText}
      etaText={installer.etaText}
    />
  {:else if installer.state.tag === 'succeeded'}
    <DoneView
      appName={installer.appName}
      canLaunch={installer.canLaunch}
      launching={installer.launching}
      onopen={() => installer.openApp()}
      ondone={() => installer.close()}
    />
  {:else if installer.incompatible}
    <IncompatibleView
      title={installer.incompatible.title}
      body={installer.incompatible.body}
      iconUrl={installer.iconUrl}
      canOpen={installer.incompatible.action === 'open'}
      onclose={() => installer.close()}
      onopen={() => installer.openInstalled()}
    />
  {:else if installer.state.tag === 'inspecting' || installer.state.tag === 'idle'}
    <div class="center">
      <Spinner size={20} />
      <span class="sr-only">{t('phase.preparing')}</span>
    </div>
  {:else if installer.error}
    {@const err = installer.error}
    <Alert title={err.message} oncancel={() => installer.close()}>
      {#snippet icon()}<AppIcon src={installer.iconUrl} size={64} />{/snippet}
      {#snippet actions()}
        {#if err.action === 'tryAgain'}
          <Button variant="primary" onclick={() => installer.retry()}>{t('action.tryAgain')}</Button
          >
          <Button onclick={() => installer.close()}>{t('action.close')}</Button>
        {:else if err.action === 'close'}
          <Button variant="primary" onclick={() => installer.close()}>{t('action.close')}</Button>
        {/if}
        {#if err.detail}
          <Disclosure summary={t('details.label')}><code>{err.code}: {err.detail}</code></Disclosure
          >
        {/if}
      {/snippet}
    </Alert>
  {/if}

  {#if defaultHandler.sheetVisible}
    <Sheet
      title={t('default.title')}
      body={t('default.body')}
      oncancel={() => defaultHandler.notNow()}
    >
      {#snippet actions()}
        <Button onclick={() => defaultHandler.notNow()}>{t('default.notNow')}</Button>
        <Button variant="primary" onclick={() => defaultHandler.makeDefault()}>
          {t('default.make')}
        </Button>
      {/snippet}
    </Sheet>
  {/if}
  {#if defaultHandler.toastVisible}
    <Toast title={t('default.toast')} ondismiss={() => {}} />
  {/if}
  {#if installer.queueNotice}
    <Toast
      title={installer.queueNotice.title}
      body={installer.queueNotice.body}
      ondismiss={() => installer.dismissQueueNotice()}
    />
  {/if}
</main>

<style>
  .install {
    position: relative;
    height: 100%;
    min-height: 100%;
  }
  .center {
    display: grid;
    place-items: center;
    height: 100%;
  }
</style>
