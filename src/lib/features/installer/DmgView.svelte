<script lang="ts">
  import { onDestroy } from 'svelte';
  import { t } from '$lib/i18n';
  import ArrowIcon from '$lib/ui/icons/ArrowIcon.svelte';
  import LockIcon from '$lib/ui/icons/LockIcon.svelte';
  import { duration, motionMs } from '$lib/ui/motion';
  import ApplicationsFolder from './ApplicationsFolder.svelte';
  import DraggableAppIcon from './DraggableAppIcon.svelte';
  import PermissionShade from './PermissionShade.svelte';

  /**
   * The "drag to Applications" screen. Presentational: the controller owns the state and
   * this view only reports intent. `oninstall` fires after the drop animation has played.
   */
  interface Props {
    appName: string;
    iconUrl?: string | null;
    /** System-wide install: show the "Needs your password" note. */
    needsPassword?: boolean;
    /** The app is already installed, so the folder says "Update". */
    isUpdate?: boolean;
    /** Controller: the install has started (keeps the icon in the folder). */
    started?: boolean;
    /** Controller: the system permission dialog is open. */
    waitingForPermission?: boolean;
    oninstall: () => void;
    /** Cancel while waiting for permission. */
    oncancel: () => void;
  }

  let {
    appName,
    iconUrl = null,
    needsPassword = false,
    isUpdate = false,
    started = false,
    waitingForPermission = false,
    oninstall,
    oncancel
  }: Props = $props();

  let tile = $state<HTMLElement | null>(null);
  let hover = $state(false);
  // True between the drop and the moment we tell the controller.
  let flying = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;

  const flown = $derived(flying || started || waitingForPermission);

  function handleDrop() {
    flying = true;
    clearTimeout(timer);
    timer = setTimeout(() => {
      oninstall();
      flying = false;
    }, motionMs(duration.flight));
  }

  onDestroy(() => clearTimeout(timer));
</script>

<div class="view">
  <div class="row">
    <div class="col">
      <DraggableAppIcon
        name={appName}
        {iconUrl}
        {flown}
        target={() => tile}
        bind:hover
        ondrop={handleDrop}
      />
      <span class="label">{appName}</span>
    </div>
    <div class="arrow"><ArrowIcon /></div>
    <ApplicationsFolder {hover} gulp={flown} {isUpdate} bind:tile />
  </div>

  <div class="hint">
    <p class="callout">{t('dmg.hint', { app: appName })}</p>
    {#if needsPassword}
      <span class="lock-note"><LockIcon /> {t('dmg.needsPassword')}</span>
    {/if}
  </div>

  {#if waitingForPermission}
    <PermissionShade {iconUrl} {oncancel} />
  {/if}
</div>

<style>
  .view {
    height: 100%;
    padding: var(--sp-6);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--gap-view);
  }
  .row {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-14);
  }
  .col {
    width: var(--col-dmg);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--gap-row);
  }
  .label {
    font-size: var(--fs-headline);
    line-height: var(--lh-headline);
    font-weight: 600;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .arrow {
    width: var(--w-dmg-arrow);
    height: var(--icon-dmg);
    display: flex;
    align-items: center;
  }
  .hint {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--sp-2);
  }
  .callout {
    font-size: var(--fs-callout);
    line-height: var(--lh-callout);
    color: var(--label-tertiary);
  }
  .lock-note {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
    font-size: var(--fs-footnote);
    line-height: var(--lh-footnote);
    color: var(--label-secondary);
  }
</style>
