<script lang="ts">
  // Install window: drives S6's views from InstallerController. No logic here beyond wiring.
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
</script>

<main class="install" data-state={installer.state.tag}>
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
  {:else if installer.state.tag === 'inspecting'}
    <div class="center"><Spinner size={20} label={t('phase.preparing')} /></div>
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
  }
  .center {
    display: grid;
    place-items: center;
    height: 100%;
  }
</style>
