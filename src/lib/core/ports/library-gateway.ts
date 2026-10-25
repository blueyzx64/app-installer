import type { InstallEvent, InstalledApp, UninstallOutcome } from '../domain/types';

/**
 * What removing an app will touch (F-42). Contracts v2.
 * TODO(v2:S4): swap for the generated type once bindings carry `UninstallPlan`.
 */
export interface UninstallPlan {
  /** Other items that only this app used and go with it. */
  alsoRemoves: number;
  /** A password prompt will appear. */
  needsPermission: boolean;
}

export interface LibraryGateway {
  listInstalled(): Promise<InstalledApp[]>;
  /** Tells the confirm alert what removal will do. Never changes anything. */
  uninstallPlan(appId: string): Promise<UninstallPlan>;
  uninstall(appId: string, onEvent: (e: InstallEvent) => void): Promise<UninstallOutcome>;
  launchApp(appId: string): Promise<void>;
}
