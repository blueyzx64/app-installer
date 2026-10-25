import { describe, expect, it, vi } from 'vitest';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({
  invoke,
  Channel: class {
    onmessage: (m: unknown) => void = () => {};
  }
}));
vi.mock('./bindings', () => ({ commands: {} }));

import { TauriLibraryGateway } from './library-gateway';

describe('TauriLibraryGateway.uninstallPlan', () => {
  it('calls uninstall_plan with the app id', async () => {
    invoke.mockResolvedValue({ alsoRemoves: 2, needsPermission: true });
    const plan = await new TauriLibraryGateway().uninstallPlan('orbit-player');
    expect(invoke).toHaveBeenCalledWith('uninstall_plan', { appId: 'orbit-player' });
    expect(plan).toEqual({ alsoRemoves: 2, needsPermission: true });
  });
  it('turns failures into CommandError', async () => {
    invoke.mockRejectedValue({ code: 'notFound', detail: null });
    await expect(new TauriLibraryGateway().uninstallPlan('x')).rejects.toMatchObject({
      code: 'notFound'
    });
  });
});
