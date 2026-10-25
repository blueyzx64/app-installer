<script lang="ts">
  import { t } from '$lib/i18n';
  import Alert from '$lib/ui/Alert.svelte';
  import AppIcon from '$lib/ui/AppIcon.svelte';
  import Button from '$lib/ui/Button.svelte';
  import type { ConfirmView } from './LibraryController.svelte';

  interface Props {
    confirm: ConfirmView;
    onconfirm: () => void;
    oncancel: () => void;
  }

  let { confirm, onconfirm, oncancel }: Props = $props();
</script>

<!-- Focus starts on Cancel: the safe choice for a destructive question. -->
<Alert title={confirm.title} message={confirm.body} {oncancel} initialFocus=".secondary">
  {#snippet icon()}
    <AppIcon src={confirm.app.iconDataUrl} size={64} />
  {/snippet}
  {#snippet actions()}
    <Button variant="destructive" disabled={!confirm.ready} onclick={onconfirm}>
      {confirm.confirmLabel}
    </Button>
    <Button onclick={oncancel}>{confirm.cancelLabel || t('trash.cancel')}</Button>
  {/snippet}
</Alert>
