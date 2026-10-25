import { describe, expect, it } from 'vitest';
import {
  appsOf,
  initialLibraryState,
  libraryMachine as m,
  type LibraryState
} from './library-machine';
import type { InstalledApp } from './types';

const mk = (id: string): InstalledApp => ({
  id,
  displayName: id,
  version: '1',
  iconDataUrl: null,
  source: 'appImage',
  installedAt: '2026-10-25T00:00:00Z',
  canLaunch: true
});
const a = mk('a');
const b = mk('b');
const listing: LibraryState = { tag: 'listing', apps: [a, b], notice: null };
const confirming: LibraryState = { tag: 'confirming', apps: [a, b], app: a };
const removing: LibraryState = { tag: 'removing', apps: [a, b], app: a };

describe('libraryMachine', () => {
  it('loading -> empty / listing', () => {
    expect(m(initialLibraryState, { type: 'LOADED', apps: [] })).toEqual({
      tag: 'empty',
      notice: null
    });
    expect(m(initialLibraryState, { type: 'LOADED', apps: [a] })).toEqual({
      tag: 'listing',
      apps: [a],
      notice: null
    });
  });
  it('load failure -> empty with notice', () => {
    const error = { code: 'unknown', detail: null } as const;
    expect(m(initialLibraryState, { type: 'LOAD_FAILED', error })).toEqual({
      tag: 'empty',
      notice: error
    });
  });
  it('listing + TRASH(app) -> confirming; unknown id ignored', () => {
    expect(m(listing, { type: 'TRASH', appId: 'a' })).toEqual(confirming);
    expect(m(listing, { type: 'TRASH', appId: 'zzz' })).toBe(listing);
  });
  it('confirming + CANCEL -> listing', () => {
    expect(m(confirming, { type: 'CANCEL' })).toEqual(listing);
  });
  it('confirming + CONFIRM -> removing', () => {
    expect(m(confirming, { type: 'CONFIRM' })).toEqual(removing);
  });
  it('removing + removed -> listing without the app', () => {
    expect(m(removing, { type: 'UNINSTALL_DONE', outcome: { kind: 'removed' } })).toEqual({
      tag: 'listing',
      apps: [b],
      notice: null
    });
  });
  it('removing the last app -> empty', () => {
    const s: LibraryState = { tag: 'removing', apps: [a], app: a };
    expect(m(s, { type: 'UNINSTALL_DONE', outcome: { kind: 'removed' } }).tag).toBe('empty');
  });
  it('removing + cancelled -> listing, all apps, no notice', () => {
    expect(m(removing, { type: 'UNINSTALL_DONE', outcome: { kind: 'cancelled' } })).toEqual(
      listing
    );
  });
  it('removing + failed -> listing with notice', () => {
    expect(
      m(removing, {
        type: 'UNINSTALL_DONE',
        outcome: { kind: 'failed', code: 'busy', detail: 'x' }
      })
    ).toEqual({ tag: 'listing', apps: [a, b], notice: { code: 'busy', detail: 'x' } });
    expect(
      m(removing, { type: 'UNINSTALL_FAILED', error: { code: 'unknown', detail: null } })
    ).toMatchObject({ tag: 'listing', notice: { code: 'unknown' } });
  });
  it('DISMISS_NOTICE clears the notice', () => {
    const n: LibraryState = { tag: 'listing', apps: [a], notice: { code: 'busy', detail: null } };
    expect(m(n, { type: 'DISMISS_NOTICE' })).toEqual({ tag: 'listing', apps: [a], notice: null });
  });
  it('LOADED refreshes a listing and keeps the notice', () => {
    const n: LibraryState = { tag: 'listing', apps: [a], notice: { code: 'busy', detail: null } };
    expect(m(n, { type: 'LOADED', apps: [b] })).toEqual({
      tag: 'listing',
      apps: [b],
      notice: { code: 'busy', detail: null }
    });
  });
  it('cannot trash while confirming or removing', () => {
    expect(m(confirming, { type: 'TRASH', appId: 'b' })).toBe(confirming);
    expect(m(removing, { type: 'TRASH', appId: 'b' })).toBe(removing);
    expect(m(removing, { type: 'CANCEL' })).toBe(removing);
    expect(m(removing, { type: 'CONFIRM' })).toBe(removing);
  });
  it('appsOf', () => {
    expect(appsOf(listing)).toEqual([a, b]);
    expect(appsOf(initialLibraryState)).toEqual([]);
  });
});
