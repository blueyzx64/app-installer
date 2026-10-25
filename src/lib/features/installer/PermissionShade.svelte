<script lang="ts">
  import { t } from '$lib/i18n';
  import Alert from '$lib/ui/Alert.svelte';
  import AppIcon from '$lib/ui/AppIcon.svelte';
  import Button from '$lib/ui/Button.svelte';
  import LockIcon from '$lib/ui/icons/LockIcon.svelte';

  /**
   * Shown while the system's own permission dialog is open. This view never asks for a password
   * (polkit owns authentication, ADR-0007). Cancel here is a plain cancel, not an error.
   */
  interface Props {
    iconUrl?: string | null;
    oncancel: () => void;
  }

  let { iconUrl = null, oncancel }: Props = $props();
</script>

<Alert title={t('perm.waiting')} message={t('perm.sub')} {oncancel} initialFocus="button">
  {#snippet icon()}
    <AppIcon src={iconUrl} size={64}>
      {#snippet badge()}<LockIcon size={14} />{/snippet}
    </AppIcon>
  {/snippet}
  {#snippet actions()}
    <Button onclick={oncancel}>{t('action.cancel')}</Button>
  {/snippet}
</Alert>
