<script lang="ts">
  // Composition root: picks the gateways (Tauri in the app, mock in a plain browser) and
  // provides them plus the feature controllers to the whole tree.
  import '$lib/ui/base.css';
  import { goto } from '$app/navigation';
  import { resolve } from '$app/paths';
  import { createControllers, provideControllers, provideGateways } from '$lib/app/context';
  import { isTauri } from '$lib/app/env';
  import { AppearanceStore } from '$lib/features/appearance/AppearanceStore.svelte';
  import { t } from '$lib/i18n';
  import { createMockGateways } from '$lib/infrastructure/mock';
  import { createTauriGateways } from '$lib/infrastructure/tauri';
  import WindowChrome from '$lib/ui/WindowChrome.svelte';

  let { children } = $props();

  const gateways = provideGateways(isTauri() ? createTauriGateways() : createMockGateways());
  // Mock mode only: e2e and manual testing drive open-file, appearance and removals from here.
  if ('controls' in gateways) {
    (globalThis as { __mock?: unknown }).__mock = gateways.controls;
  }
  const { defaultHandler, installer } = provideControllers(createControllers(gateways));
  const appearance = new AppearanceStore(gateways.appearance);

  $effect(() => {
    void defaultHandler.check();
  });

  // One subscription for the whole app (F-02, D-022): a file opened on any screen, library
  // included, goes to the installer. A busy install queues it instead.
  $effect(() => installer.start(() => void goto(resolve('/install'))));

  $effect(() => {
    void appearance.start();
    return () => appearance.stop();
  });
</script>

<WindowChrome
  title={t('app.name')}
  controls={appearance.current?.controls}
  window={gateways.window}
>
  <div class="app-shell">
    {@render children()}
  </div>
</WindowChrome>
