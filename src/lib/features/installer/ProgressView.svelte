<script lang="ts" module>
  export type ProgressPhase =
    'preparing' | 'waitingForPermission' | 'downloading' | 'installing' | 'finishing';

  /** Picks the time-left phrase key and its parameters from a number of seconds. */
  export function etaPhrase(seconds: number | null): {
    key: 'eta.seconds' | 'eta.minute' | 'eta.minutes' | 'eta.unknown';
    n?: number;
  } {
    if (seconds === null || !Number.isFinite(seconds) || seconds < 0) return { key: 'eta.unknown' };
    if (seconds < 45) return { key: 'eta.seconds' };
    if (seconds < 90) return { key: 'eta.minute' };
    return { key: 'eta.minutes', n: Math.round(seconds / 60) };
  }
</script>

<script lang="ts">
  import { t } from '$lib/i18n';
  import AppIcon from '$lib/ui/AppIcon.svelte';
  import ProgressBar from '$lib/ui/ProgressBar.svelte';

  interface Props {
    appName: string;
    iconUrl?: string | null;
    phase: ProgressPhase;
    /** 0 to 100, or null when the amount is not known. */
    percent: number | null;
    etaSeconds?: number | null;
    /** Download size, already formatted ("120 MB"). Shown only for large downloads. */
    sizeLabel?: string | null;
  }

  let {
    appName,
    iconUrl = null,
    phase,
    percent,
    etaSeconds = null,
    sizeLabel = null
  }: Props = $props();

  const phaseText = $derived.by(() => {
    switch (phase) {
      case 'preparing':
        return t('phase.preparing');
      case 'waitingForPermission':
        return t('perm.waiting');
      case 'downloading':
        return sizeLabel ? t('phase.downloadingSize', { size: sizeLabel }) : t('phase.downloading');
      case 'installing':
        return t('phase.installing', { app: appName });
      case 'finishing':
        return t('phase.finishing');
    }
  });

  const etaText = $derived.by(() => {
    if (phase === 'finishing') return '';
    const { key, n } = etaPhrase(etaSeconds);
    return key === 'eta.minutes' ? t(key, { n: n ?? 0 }) : t(key);
  });
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
