<script lang="ts">
  import { t } from '$lib/i18n';
  import AppIcon from '$lib/ui/AppIcon.svelte';
  import Button from '$lib/ui/Button.svelte';

  interface Props {
    onopenpackage: () => void;
  }

  let { onopenpackage }: Props = $props();
</script>

<div class="empty">
  <AppIcon size={96} />
  <h2>{t('library.empty.title')}</h2>
  <p>{t('library.empty')}</p>
  <Button variant="primary" onclick={onopenpackage}>{t('library.open')}</Button>
</div>

<style>
  .empty {
    height: 100%;
    padding: var(--sp-8);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--sp-3);
    text-align: center;
  }
  h2 {
    margin-top: var(--sp-2);
    font-size: var(--fs-title-2);
    line-height: var(--lh-title-2);
    font-weight: 700;
    animation: var(--anim-rise);
  }
  p {
    max-width: 320px;
    line-height: 1.5;
    color: var(--label-secondary);
    animation: var(--anim-rise);
  }
  .empty :global(.btn) {
    margin-top: var(--sp-2);
  }
</style>
