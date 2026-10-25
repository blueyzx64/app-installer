// @vitest-environment jsdom
import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import type { InstalledApp } from '$lib/core/domain/types';
import AppGrid from './AppGrid.svelte';
import AppTile from './AppTile.svelte';
import LibraryEmpty from './LibraryEmpty.svelte';
import TrashConfirmAlert from './TrashConfirmAlert.svelte';
import type { ConfirmView } from './LibraryController.svelte';

const mk = (id: string, source: InstalledApp['source'] = 'appImage', canLaunch = true) =>
  ({
    id,
    displayName: `App ${id}`,
    version: '1',
    iconDataUrl: null,
    source,
    installedAt: '2026-10-25T00:00:00Z',
    canLaunch
  }) satisfies InstalledApp;

const tileProps = (over = {}) => ({
  app: mk('a'),
  trashLabel: 'Move to Trash',
  onlaunch: vi.fn(),
  ontrash: vi.fn(),
  onmenu: vi.fn(),
  ...over
});

describe('LibraryEmpty', () => {
  it('offers Open a Package…', async () => {
    const onopenpackage = vi.fn();
    render(LibraryEmpty, { onopenpackage });
    expect(screen.getByText('Apps you install will appear here.')).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Open a Package…' }));
    expect(onopenpackage).toHaveBeenCalledOnce();
  });
});

describe('AppTile', () => {
  it('shows the name; double click and Enter open the app', async () => {
    const p = tileProps();
    render(AppTile, p);
    const tile = screen.getByRole('button', { name: /App a/ });
    await fireEvent.dblClick(tile);
    await fireEvent.keyDown(tile, { key: 'Enter' });
    expect(p.onlaunch).toHaveBeenCalledTimes(2);
  });
  it('does not launch an app that cannot launch', async () => {
    const p = tileProps({ app: mk('a', 'appImage', false) });
    render(AppTile, p);
    await fireEvent.dblClick(screen.getByRole('button', { name: /App a/ }));
    expect(p.onlaunch).not.toHaveBeenCalled();
  });
  it('Delete asks to trash; right click and the menu key open the menu', async () => {
    const p = tileProps();
    render(AppTile, p);
    const tile = screen.getByRole('button', { name: /App a/ });
    await fireEvent.keyDown(tile, { key: 'Delete' });
    expect(p.ontrash).toHaveBeenCalledWith('a');
    await fireEvent.contextMenu(tile);
    await fireEvent.keyDown(tile, { key: 'ContextMenu' });
    expect(p.onmenu).toHaveBeenCalledTimes(2);
    expect(p.onmenu).toHaveBeenCalledWith('a');
  });
  it('menu lists Open and the trash action', async () => {
    const p = tileProps({ menuOpen: true });
    render(AppTile, p);
    expect(screen.getByRole('menu', { name: 'Options for App a' })).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('menuitem', { name: 'Open' }));
    expect(p.onlaunch).toHaveBeenCalledWith('a');
    await fireEvent.click(screen.getByRole('menuitem', { name: 'Move to Trash' }));
    expect(p.ontrash).toHaveBeenCalledWith('a');
    expect(p.onmenu).toHaveBeenCalledWith(null);
  });
  it('menu hides Open when the app cannot launch; Escape closes it', async () => {
    const p = tileProps({ menuOpen: true, app: mk('a', 'systemPackage', false) });
    render(AppTile, p);
    expect(screen.queryByRole('menuitem', { name: 'Open' })).toBeNull();
    await fireEvent.keyDown(screen.getByRole('menu'), { key: 'Escape' });
    expect(p.onmenu).toHaveBeenCalledWith(null);
  });
  it('a busy tile ignores input', async () => {
    const p = tileProps({ busy: true });
    render(AppTile, p);
    const tile = screen.getByRole('button', { name: /App a/ });
    expect(tile).toBeDisabled();
    await fireEvent.keyDown(tile, { key: 'Delete' });
    expect(p.ontrash).not.toHaveBeenCalled();
  });
});

describe('AppGrid', () => {
  const apps = [mk('a'), mk('b'), mk('c', 'systemPackage')];
  it('has one tab stop and arrow keys move focus', async () => {
    render(AppGrid, { apps, onlaunch: vi.fn(), ontrash: vi.fn() });
    const tiles = screen.getAllByRole('button');
    expect(tiles.filter((t) => t.tabIndex === 0)).toHaveLength(1);
    tiles[0]!.focus();
    await fireEvent.keyDown(tiles[0]!, { key: 'ArrowRight' });
    expect(tiles[1]).toHaveFocus();
    await fireEvent.keyDown(tiles[1]!, { key: 'End' });
    expect(tiles[2]).toHaveFocus();
    await fireEvent.keyDown(tiles[2]!, { key: 'ArrowRight' });
    expect(tiles[2]).toHaveFocus();
    await fireEvent.keyDown(tiles[2]!, { key: 'Home' });
    expect(tiles[0]).toHaveFocus();
    expect(tiles[0]!.tabIndex).toBe(0);
    expect(tiles[2]!.tabIndex).toBe(-1);
  });
  it('labels the trash action by app kind in the menu', async () => {
    const ontrash = vi.fn();
    render(AppGrid, { apps, onlaunch: vi.fn(), ontrash });
    const tiles = screen.getAllByRole('button');
    await fireEvent.contextMenu(tiles[2]!);
    await fireEvent.click(await screen.findByRole('menuitem', { name: 'Remove' }));
    expect(ontrash).toHaveBeenCalledWith('c');
  });
  it('dims the tile being removed', () => {
    render(AppGrid, { apps, removingId: 'b', onlaunch: vi.fn(), ontrash: vi.fn() });
    expect(screen.getByRole('button', { name: /App b/ })).toBeDisabled();
    expect(screen.getByRole('button', { name: /App a/ })).toBeEnabled();
  });
});

describe('TrashConfirmAlert', () => {
  const view = (over: Partial<ConfirmView> = {}): ConfirmView => ({
    app: mk('c', 'systemPackage'),
    title: 'Remove “App c”?',
    body: 'You’ll be asked for your password. This also removes 2 other items that only App c used.',
    confirmLabel: 'Remove',
    cancelLabel: 'Cancel',
    system: true,
    ready: true,
    ...over
  });
  it('shows the message and confirms or cancels', async () => {
    const onconfirm = vi.fn();
    const oncancel = vi.fn();
    render(TrashConfirmAlert, { confirm: view(), onconfirm, oncancel });
    expect(screen.getByRole('alertdialog', { name: 'Remove “App c”?' })).toBeInTheDocument();
    expect(screen.getByText(/also removes 2 other items/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Cancel' })).toHaveFocus();
    await fireEvent.click(screen.getByRole('button', { name: 'Remove' }));
    expect(onconfirm).toHaveBeenCalledOnce();
    await fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(oncancel).toHaveBeenCalledOnce();
  });
  it('Escape cancels', async () => {
    const oncancel = vi.fn();
    render(TrashConfirmAlert, { confirm: view(), onconfirm: vi.fn(), oncancel });
    await fireEvent.keyDown(screen.getByRole('alertdialog'), { key: 'Escape' });
    expect(oncancel).toHaveBeenCalled();
  });
  it('Remove waits until the plan is known', () => {
    render(TrashConfirmAlert, {
      confirm: view({ ready: false }),
      onconfirm: vi.fn(),
      oncancel: vi.fn()
    });
    expect(screen.getByRole('button', { name: 'Remove' })).toBeDisabled();
  });
});
