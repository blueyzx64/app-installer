// R8 re-verify (library cases), kept as regression tests.
import { describe, expect, it } from 'vitest';
import { createMockGateways } from '$lib/infrastructure/mock';
import type { InstalledApp } from '$lib/core/domain/types';
import { LibraryController } from './library/LibraryController.svelte';

function deferred<T>() {
  let resolve!: (v: T) => void;
  const p = new Promise<T>((r) => (resolve = r));
  return { p, resolve };
}

describe('review_r8 reverify', () => {
  // F-43 + F-41: a list fetched BEFORE a removal finished must not bring the removed app back.
  // loadSeq only orders loads against loads, not against removals.
  it('review_r8_stale_load_does_not_resurrect_removed_app', async () => {
    const gw = createMockGateways({ scenario: 'appimage', launchFile: null });
    const c = new LibraryController(gw);
    await c.load();
    const before = c.apps.map((a) => a.id);
    const victim = c.apps.find((a) => a.source === 'appImage')!;
    // Arrival reload in flight (slow D-Bus), returns the list as it was at call time.
    const snapshot: InstalledApp[] = [...c.apps];
    const slow = deferred<InstalledApp[]>();
    gw.library.listInstalled = () => slow.p;
    const pending = c.load();
    await c.requestTrash(victim.id);
    await c.confirmTrash();
    expect(c.apps.map((a) => a.id)).not.toContain(victim.id);
    slow.resolve(snapshot);
    await pending;
    expect(before).toContain(victim.id);
    expect(c.apps.map((a) => a.id)).not.toContain(victim.id);
  });

  // F-43: a reload that lands while the confirm alert is open is dropped; after Cancel the
  // list still shows an app that was removed outside the installer.
  it('review_r8_reload_during_confirm_is_not_lost', async () => {
    const gw = createMockGateways({ scenario: 'appimage', launchFile: null });
    const c = new LibraryController(gw);
    await c.load();
    const [first, gone] = c.apps;
    await c.requestTrash(first!.id);
    const fresh = c.apps.filter((a) => a.id !== gone!.id);
    gw.library.listInstalled = async () => fresh;
    await c.load(); // afterNavigate on return to /library
    c.cancelTrash();
    expect(c.apps.map((a) => a.id)).not.toContain(gone!.id);
  });
});
