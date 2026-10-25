// @vitest-environment jsdom
// R8 review proofs for the library (kept as regression tests).
import { describe, expect, it, vi } from 'vitest';
import { createMockGateways } from '$lib/infrastructure/mock';
import { LibraryController } from './library/LibraryController.svelte';

describe('review_r8 library', () => {
  // F-42: confirm pressed before the plan arrived must not start a removal behind a stuck alert.
  it('review_r8_confirm_before_plan_does_not_uninstall', async () => {
    const gw = createMockGateways({ scenario: 'rpm', launchFile: null });
    const c = new LibraryController(gw);
    await c.load();
    gw.library.uninstallPlan = () => new Promise(() => {}); // never resolves
    const spy = vi.spyOn(gw.library, 'uninstall');
    void c.requestTrash('orbit-player');
    await c.confirmTrash();
    expect(spy).not.toHaveBeenCalled();
  });

  // F-42: when the plan cannot be fetched, user is told nothing about extra items
  // yet removal may still take them. Alert must not look like "no extras".
  it('review_r8_plan_failure_is_not_presented_as_zero_extras', async () => {
    const gw = createMockGateways({ scenario: 'rpm', launchFile: null });
    gw.library.uninstallPlan = () => Promise.reject({ code: 'backendUnavailable', detail: null });
    const c = new LibraryController(gw);
    await c.load();
    await c.requestTrash('orbit-player');
    // Unknown plan: generic warning, never the zero-extras text. Confirm stays possible.
    expect(c.confirm?.body).toBe(
      'You’ll be asked for your password. This may also remove other items that only Orbit Player used.'
    );
    expect(c.confirm?.ready).toBe(true);
  });
});
