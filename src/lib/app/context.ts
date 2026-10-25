import { getContext, setContext } from 'svelte';
import type { Gateways } from '$lib/core/ports';
import { DefaultHandlerController } from '$lib/features/default-handler/DefaultHandlerController.svelte';
import { InstallerController } from '$lib/features/installer/InstallerController.svelte';
import { LibraryController } from '$lib/features/library/LibraryController.svelte';

export type { Gateways } from '$lib/core/ports';

export interface Controllers {
  installer: InstallerController;
  library: LibraryController;
  defaultHandler: DefaultHandlerController;
}

const GATEWAYS = Symbol('gateways');
const CONTROLLERS = Symbol('controllers');

export function provideGateways(gateways: Gateways): Gateways {
  setContext(GATEWAYS, gateways);
  return gateways;
}

export function useGateways(): Gateways {
  const g = getContext<Gateways | undefined>(GATEWAYS);
  if (!g) throw new Error('useGateways() called outside a component tree with provideGateways()');
  return g;
}

export function createControllers(gateways: Gateways): Controllers {
  const installer = new InstallerController(gateways);
  return {
    installer,
    library: new LibraryController(gateways),
    defaultHandler: new DefaultHandlerController(gateways, () => installer.state.tag === 'ready')
  };
}

export function provideControllers(controllers: Controllers): Controllers {
  setContext(CONTROLLERS, controllers);
  return controllers;
}

export function useControllers(): Controllers {
  const c = getContext<Controllers | undefined>(CONTROLLERS);
  if (!c)
    throw new Error('useControllers() called outside a component tree with provideControllers()');
  return c;
}
