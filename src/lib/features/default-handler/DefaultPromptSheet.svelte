<script lang="ts">
  import { onMount } from 'svelte';
  import { CONFIRMED_MS } from '$lib/core/domain/default-prompt-machine';
  import { t } from '$lib/i18n';
  import AppIcon from '$lib/ui/AppIcon.svelte';
  import Button from '$lib/ui/Button.svelte';
  import Sheet from '$lib/ui/Sheet.svelte';
  import Toast from '$lib/ui/Toast.svelte';
  import CheckIcon from '$lib/ui/icons/CheckIcon.svelte';
  import type { DefaultHandlerController } from './DefaultHandlerController.svelte';

  /**
   * First-launch "make Installer the default" sheet plus its confirmation toast (F-50).
   * Visibility is the controller's call: it needs `canShow` from the installer route.
   * Mount inside the positioned window body so Sheet and Toast anchor to it.
   */
  interface Props {
    controller: DefaultHandlerController;
  }

  let { controller }: Props = $props();

  onMount(() => {
    void controller.check();
  });
</script>

{#if controller.sheetVisible}
  <Sheet title={t('default.title')} body={t('default.body')} oncancel={() => controller.notNow()}>
    {#snippet icon()}<AppIcon size={64} />{/snippet}
    {#snippet actions()}
      <Button fill onclick={() => controller.notNow()}>{t('default.notNow')}</Button>
      <Button
        fill
        variant="primary"
        busy={controller.state.tag === 'applying'}
        onclick={() => controller.makeDefault()}
      >
        {t('default.make')}
      </Button>
    {/snippet}
  </Sheet>
{/if}

{#if controller.toastVisible}
  <Toast
    title={t('default.toast')}
    duration={CONFIRMED_MS}
    ondismiss={() => controller.dismissToast()}
  >
    {#snippet icon()}<CheckIcon size={20} />{/snippet}
  </Toast>
{/if}
