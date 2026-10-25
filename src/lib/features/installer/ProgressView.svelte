<script lang="ts">
  import AppIcon from '$lib/ui/AppIcon.svelte';
  import ProgressBar from '$lib/ui/ProgressBar.svelte';
  import { t } from '$lib/i18n';

  /** Renders only the text the controller gives it. */
  interface Props {
    appName: string;
    iconUrl?: string | null;
    /** 0 to 100, or null when the amount is not known (indeterminate bar). */
    percent: number | null;
    phaseText: string;
    etaText?: string;
  }

  let { appName, iconUrl = null, percent, phaseText, etaText = '' }: Props = $props();
</script>

<div class="view">
  <AppIcon src={iconUrl} size={72} />
  <h2>{t('progress.title', { app: appName })}</h2>
  <div class="wrap">
    <ProgressBar value={percent} label={t('progress.label')} />
    <div class="meta">
      <span class="phase" aria-live="polite">{phaseText}</span>
      <span class="eta tabular">{etaText}</span>
    </div>
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
    gap: var(--sp-4);
    animation: var(--anim-rise);
  }
  h2 {
    font-size: var(--fs-title-3);
    line-height: var(--lh-title-3);
    font-weight: 700;
  }
  .wrap {
    width: 360px;
    max-width: 100%;
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }
  .meta {
    display: flex;
    justify-content: space-between;
    gap: var(--sp-3);
    font-size: var(--fs-footnote);
    line-height: var(--lh-footnote);
  }
  .phase {
    color: var(--label-tertiary);
  }
  .eta {
    color: var(--label-secondary);
    text-align: right;
  }
</style>
