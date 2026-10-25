import { describe, expect, it } from 'vitest';
import {
  appsOf,
  initialLibraryState,
  libraryMachine as m,
  type LibraryEvent,
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
const plan = { alsoRemoves: 2, needsPermission: true };
const confirmingNoPlan: LibraryState = { tag: 'confirming', apps: [a, b], app: a, plan: null };
const confirming: LibraryState = { tag: 'confirming', apps: [a, b], app: a, plan };
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
    expect(m(listing, { type: 'TRASH', appId: 'a' })).toEqual(confirmingNoPlan);
    expect(m(listing, { type: 'TRASH', appId: 'zzz' })).toBe(listing);
  });
  it('confirming + CANCEL -> listing', () => {
    expect(m(confirming, { type: 'CANCEL' })).toEqual(listing);
  });
  it('PLAN_LOADED fills the plan; a plan for another app is ignored', () => {
    expect(m(confirmingNoPlan, { type: 'PLAN_LOADED', appId: 'a', plan })).toEqual(confirming);
    expect(m(confirmingNoPlan, { type: 'PLAN_LOADED', appId: 'b', plan })).toBe(confirmingNoPlan);
  });
  it('PLAN_FAILED marks the plan failed and lets CONFIRM go on', () => {
    const failed = m(confirmingNoPlan, { type: 'PLAN_FAILED', appId: 'a' });
    expect(failed).toMatchObject({ tag: 'confirming', plan: 'failed' });
    expect(m(failed, { type: 'CONFIRM' }).tag).toBe('removing');
    expect(m(confirmingNoPlan, { type: 'PLAN_FAILED', appId: 'b' })).toBe(confirmingNoPlan);
    expect(m(listing, { type: 'PLAN_FAILED', appId: 'a' })).toBe(listing);
  });
  it('PLAN_LOADED outside confirming is ignored', () => {
    const ev = { type: 'PLAN_LOADED', appId: 'a', plan } as const;
    expect(m(listing, ev)).toBe(listing);
    expect(m(removing, ev)).toBe(removing);
    expect(m(initialLibraryState, ev)).toBe(initialLibraryState);
    expect(m({ tag: 'empty', notice: null }, ev)).toEqual({ tag: 'empty', notice: null });
  });
  it('CONFIRM waits for the plan', () => {
    expect(m(confirmingNoPlan, { type: 'CONFIRM' })).toBe(confirmingNoPlan);
  });
  it('confirming without a plan can still be cancelled', () => {
    expect(m(confirmingNoPlan, { type: 'CANCEL' })).toEqual(listing);
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
  it('ignores events that do not belong to the state', () => {
    const all: LibraryEvent[] = [
      { type: 'LOADED', apps: [a] },
      { type: 'LOAD_FAILED', error: { code: 'unknown', detail: null } },
      { type: 'TRASH', appId: 'a' },
      { type: 'CANCEL' },
      { type: 'CONFIRM' },
      { type: 'UNINSTALL_DONE', outcome: { kind: 'removed' } },
      { type: 'UNINSTALL_FAILED', error: { code: 'unknown', detail: null } },
      { type: 'DISMISS_NOTICE' }
    ];
    const loadingOk = ['LOADED', 'LOAD_FAILED'];
    for (const ev of all) {
      if (!loadingOk.includes(ev.type)) {
        expect(m(initialLibraryState, ev)).toBe(initialLibraryState);
      }
    }
    const empty: LibraryState = { tag: 'empty', notice: null };
    expect(m(empty, { type: 'LOADED', apps: [a] }).tag).toBe('listing');
    expect(m(empty, { type: 'TRASH', appId: 'a' })).toBe(empty);
    expect(m(empty, { type: 'CONFIRM' })).toBe(empty);
    const emptyN: LibraryState = { tag: 'empty', notice: { code: 'busy', detail: null } };
    expect(m(emptyN, { type: 'DISMISS_NOTICE' })).toEqual(empty);
    expect(m(listing, { type: 'CONFIRM' })).toBe(listing);
    expect(m(listing, { type: 'CANCEL' })).toBe(listing);
    expect(m(confirming, { type: 'UNINSTALL_DONE', outcome: { kind: 'removed' } })).toBe(
      confirming
    );
    expect(m(removing, { type: 'DISMISS_NOTICE' })).toBe(removing);
    expect(m(removing, { type: 'LOADED', apps: [] })).toBe(removing);
  });
  it('appsOf', () => {
    expect(appsOf(listing)).toEqual([a, b]);
    expect(appsOf(initialLibraryState)).toEqual([]);
  });
});
