<script lang="ts" module>
  export type IncompatibleKind =
    | 'wrongFamily.debBased'
    | 'wrongFamily.rpmBased'
    | 'wrongArch'
    | 'alreadyInstalled.same'
    | 'alreadyInstalled.older'
    | 'missingRuntime.fuse2'
    | 'unsupported';
</script>

<script lang="ts">
  import { t } from '$lib/i18n';
  import AppIcon from '$lib/ui/AppIcon.svelte';
  import Button from '$lib/ui/Button.svelte';
  import { focusOnMount } from '$lib/ui/focus';

  /** "Can't install this" and "already installed" screen. Calm, no technical words. */
  interface Props {
    /** Used when `title` is not given. */
    kind?: IncompatibleKind;
    /** Text already resolved by the controller. Wins over `kind`. */
    title?: string;
    body?: string | null;
    appName: string;
    iconUrl?: string | null;
    /** Offered only for "already installed, same version". */
    canOpen?: boolean;
    onclose: () => void;
    onopen?: () => void;
  }

  let {
    kind,
    title: titleOverride,
    body: bodyOverride,
    appName,
    iconUrl = null,
    canOpen = false,
    onclose,
    onopen
  }: Props = $props();

  const title = $derived(
    titleOverride ?? (kind ? t(`compat.${kind}.title`, { app: appName }) : '')
  );
  const body = $derived(
    bodyOverride !== undefined ? bodyOverride : kind ? t(`compat.${kind}.body`) : ''
  );
  // canOpen is the controller's decision (action === 'open'); with only `kind`, restrict to same-version.
  const offerOpen = $derived(
    canOpen && !!onopen && (kind === undefined || kind === 'alreadyInstalled.same')
  );

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
    max-width: 420px;
    font-size: var(--fs-title-3);
    line-height: var(--lh-title-3);
    font-weight: 700;
  }
  p {
    max-width: 380px;
    line-height: 1.5;
    color: var(--label-secondary);
  }
  .buttons {
    display: flex;
    gap: 10px;
    margin-top: 10px;
  }
</style>
