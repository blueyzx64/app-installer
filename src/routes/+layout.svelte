<script lang="ts">
  // Composition root: picks the gateways (Tauri in the app, mock in a plain browser) and
  // provides them plus the feature controllers to the whole tree.
  import { createControllers, provideControllers, provideGateways } from '$lib/app/context';
  import { isTauri } from '$lib/app/env';
  import { createMockGateways } from '$lib/infrastructure/mock';
  import { createTauriGateways } from '$lib/infrastructure/tauri';

  let { children } = $props();

  const gateways = provideGateways(isTauri() ? createTauriGateways() : createMockGateways());
  const { defaultHandler } = provideControllers(createControllers(gateways));

  $effect(() => {
    void defaultHandler.check();
  });
</script>

<!-- TODO(S6/S7): replace this wrapper with <WindowChrome> around the slot. -->
<div class="app-shell" data-window-chrome-slot>
  {@render children()}
</div>
