import { beforeEach, describe, expect, it, vi } from 'vitest';

const commands = vi.hoisted(() => ({
  takeLaunchFile: vi.fn(),
  openPackage: vi.fn(),
  install: vi.fn(),
  cancelInstall: vi.fn(),
  listInstalled: vi.fn(),
  uninstall: vi.fn(),
  launchApp: vi.fn(),
  defaultHandlerStatus: vi.fn(),
  makeDefaultHandler: vi.fn(),
  dismissDefaultPrompt: vi.fn(),
  getAppearance: vi.fn()
}));
const listeners = vi.hoisted(() => new Map<string, (e: { payload: unknown }) => void>());
const unlisten = vi.hoisted(() => vi.fn());

vi.mock('./bindings', () => ({ commands }));
vi.mock('@tauri-apps/api/core', () => ({
  Channel: class {
    onmessage: (m: unknown) => void = () => {};
  }
}));
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (name: string, cb: (e: { payload: unknown }) => void) => {
    listeners.set(name, cb);
    return unlisten;
  })
}));

import { TauriAppearanceGateway } from './appearance-gateway';
import { TauriDefaultsGateway } from './defaults-gateway';
import { TauriInstallerGateway } from './installer-gateway';
import { TauriLibraryGateway } from './library-gateway';

beforeEach(() => {
  Object.values(commands).forEach((f) => f.mockReset());
  listeners.clear();
  unlisten.mockReset();
});

describe('TauriInstallerGateway', () => {
  it('passes args through and forwards channel messages', async () => {
    commands.install.mockImplementation(
      async (_id: string, ch: { onmessage: (m: unknown) => void }) => {
        ch.onmessage({ kind: 'progress', phase: 'installing', percent: 5, etaSecs: null });
        return { kind: 'cancelled' };
      }
    );
    const events: unknown[] = [];
    const out = await new TauriInstallerGateway().install('p1', (e) => events.push(e));
    expect(commands.install.mock.calls[0]?.[0]).toBe('p1');
    expect(events).toHaveLength(1);
    expect(out).toEqual({ kind: 'cancelled' });
  });
  it('rejects with a CommandError even for plain failures', async () => {
    commands.openPackage.mockRejectedValue({ code: 'corruptFile', detail: 'x' });
    await expect(new TauriInstallerGateway().openPackage('/a')).rejects.toEqual({
      code: 'corruptFile',
      detail: 'x'
    });
    commands.openPackage.mockRejectedValue('weird');
    await expect(new TauriInstallerGateway().openPackage('/a')).rejects.toEqual({
      code: 'unknown',
      detail: 'weird'
    });
  });
  it('void commands resolve undefined', async () => {
    commands.cancelInstall.mockResolvedValue(null);
    expect(await new TauriInstallerGateway().cancelInstall('p1')).toBeUndefined();
  });
  it('onOpenFile maps payload and unsubscribes, even before listen resolves', async () => {
    const g = new TauriInstallerGateway();
    const cb = vi.fn();
    const off = g.onOpenFile(cb);
    await Promise.resolve();
    listeners.get('app://open-file')?.({ payload: { path: '/f' } });
    expect(cb).toHaveBeenCalledWith('/f');
    off();
    expect(unlisten).toHaveBeenCalledOnce();

    const early = g.onOpenFile(cb);
    early();
    await Promise.resolve();
    await Promise.resolve();
    expect(unlisten).toHaveBeenCalledTimes(2);
  });
});

describe('other gateways', () => {
  it('library', async () => {
    commands.listInstalled.mockResolvedValue([]);
    commands.launchApp.mockResolvedValue(null);
    const g = new TauriLibraryGateway();
    expect(await g.listInstalled()).toEqual([]);
    expect(await g.launchApp('a')).toBeUndefined();
    expect(commands.launchApp).toHaveBeenCalledWith('a');
  });
  it('defaults', async () => {
    commands.defaultHandlerStatus.mockResolvedValue({ isDefault: false, askedBefore: false });
    commands.makeDefaultHandler.mockResolvedValue(null);
    commands.dismissDefaultPrompt.mockResolvedValue(null);
    const g = new TauriDefaultsGateway();
    expect((await g.status()).isDefault).toBe(false);
    await g.makeDefault();
    await g.dismissPrompt();
    expect(commands.dismissDefaultPrompt).toHaveBeenCalled();
  });
  it('appearance listens to appearance://changed', async () => {
    commands.getAppearance.mockResolvedValue({ colorScheme: 'dark' });
    const g = new TauriAppearanceGateway();
    expect(await g.get()).toEqual({ colorScheme: 'dark' });
    const cb = vi.fn();
    g.onChange(cb);
    await Promise.resolve();
    listeners.get('appearance://changed')?.({ payload: { colorScheme: 'light' } });
    expect(cb).toHaveBeenCalledWith({ colorScheme: 'light' });
  });
});
