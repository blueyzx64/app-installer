<script lang="ts">
  // Test-only: provides gateways and controllers around the library route.
  import { createControllers, provideControllers, provideGateways } from '$lib/app/context';
  import type { Gateways } from '$lib/core/ports';
  import Page from '../../../routes/library/+page.svelte';

  let { gateways }: { gateways: Gateways } = $props();
  // svelte-ignore state_referenced_locally
  provideGateways(gateways);
  // svelte-ignore state_referenced_locally
  provideControllers(createControllers(gateways));
</script>

<Page />
