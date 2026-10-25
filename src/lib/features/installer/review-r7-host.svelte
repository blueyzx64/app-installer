<script lang="ts">
  // Test-only (R7 review): provides gateways and controllers around the install route.
  import { createControllers, provideControllers, provideGateways } from '$lib/app/context';
  import type { Gateways } from '$lib/core/ports';
  import Page from '../../../routes/install/[[previewId]]/+page.svelte';

  let { gateways, onready }: { gateways: Gateways; onready?: (c: unknown) => void } = $props();
  // svelte-ignore state_referenced_locally
  provideGateways(gateways);
  // svelte-ignore state_referenced_locally
  const c = provideControllers(createControllers(gateways));
  // svelte-ignore state_referenced_locally
  onready?.(c);
</script>

<Page />
