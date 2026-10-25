import { describe, expect, it, vi } from 'vitest';

const uninstallPlan = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({
  Channel: class {
    onmessage: (m: unknown) => void = () => {};
  }
}));
vi.mock('./bindings', () => ({ commands: { uninstallPlan } }));

import { TauriLibraryGateway } from './library-gateway';

describe('TauriLibraryGateway.uninstallPlan', () => {
  it('calls uninstall_plan with the app id', async () => {
    uninstallPlan.mockResolvedValue({ alsoRemoves: 2, needsPermission: true });
    const plan = await new TauriLibraryGateway().uninstallPlan('orbit-player');
    expect(uninstallPlan).toHaveBeenCalledWith('orbit-player');
    expect(plan).toEqual({ alsoRemoves: 2, needsPermission: true });
  });
  it('turns failures into CommandError', async () => {
    uninstallPlan.mockRejectedValue({ code: 'notFound', detail: null });
    await expect(new TauriLibraryGateway().uninstallPlan('x')).rejects.toMatchObject({
      code: 'notFound'
    });
  });
});
