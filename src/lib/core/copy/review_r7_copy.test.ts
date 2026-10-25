// R7 review: walk every reachable enum value through copy.ts; keys must exist, no {param} left.
import { describe, expect, it } from 'vitest';
import en from '../../i18n/en.json';
import type { Compatibility, ErrorCode, InstallPhase } from '../domain/types';
import {
  compatCopy,
  compatKind,
  errorCopy,
  etaCopy,
  phaseCopy,
  removeErrorCopy,
  text,
  trashCopy,
  type Msg
} from './copy';

const ERROR_CODES: ErrorCode[] = [
  'permissionDenied',
  'offline',
  'missingDependencies',
  'conflicts',
  'notEnoughSpace',
  'corruptFile',
  'untrustedSource',
  'backendUnavailable',
  'notFound',
  'busy',
  'unknown'
];
const PHASES: InstallPhase[] = [
  'preparing',
  'waitingForPermission',
  'downloading',
  'installing',
  'finishing'
];
const COMPATS: Compatibility[] = [
  { kind: 'ok' },
  { kind: 'wrongFamily', madeFor: 'debBased' },
  { kind: 'wrongFamily', madeFor: 'rpmBased' },
  { kind: 'wrongArch', packageArch: 'arm64', hostArch: 'x86_64' },
  { kind: 'alreadyInstalled', relation: 'same', installedVersion: '1' },
  { kind: 'alreadyInstalled', relation: 'older', installedVersion: '1' },
  { kind: 'alreadyInstalled', relation: 'newer', installedVersion: '1' },
  { kind: 'missingRuntime', what: 'fuse2' },
  { kind: 'unsupported', reason: 'appImageType1' },
  { kind: 'unsupported', reason: 'dwarfs' },
  { kind: 'unsupported', reason: 'unknownFormat' }
];

function check(m: Msg | null, where: string) {
  if (!m) return;
  expect((en as Record<string, string>)[m.key], `${where}: missing key ${m.key}`).toBeDefined();
  const out = text(m);
  expect(out, `${where}: unfilled placeholder in "${out}"`).not.toMatch(/\{\w+\}/);
  expect(out, `${where}: key leaked`).not.toBe(m.key);
}

describe('review_r7 copy walk', () => {
  it('phases', () => {
    for (const p of PHASES)
      for (const plan of [undefined, { extraPackages: 1, downloadBytes: 99e6 }])
        check(phaseCopy(p, { app: 'A', plan }), `phase ${p}`);
  });
  it('eta', () => {
    for (const s of [null, -1, NaN, 0, 45, 46, 90, 91, 3600]) check(etaCopy(s), `eta ${s}`);
  });
  it('eta thresholds', () => {
    expect(text(etaCopy(45))).toBe('A few seconds left');
    expect(text(etaCopy(46))).toBe('About a minute left');
    expect(text(etaCopy(90))).toBe('About a minute left');
    expect(text(etaCopy(91))).toBe('About 2 minutes left');
  });
  it('compat', () => {
    for (const c of COMPATS) {
      const cc = compatCopy(c, 'A');
      check(cc?.title ?? null, c.kind);
      check(cc?.body ?? null, c.kind);
      expect(compatKind(c) === null).toBe(cc === null);
    }
  });
  it('errors (with app name)', () => {
    for (const c of ERROR_CODES)
      for (const sp of [undefined, 5e9]) {
        check(errorCopy(c, { app: 'A', spaceBytes: sp }).message, `error ${c}`);
        check(removeErrorCopy(c, 'A'), `remove ${c}`);
      }
  });
  it('trash', () => {
    for (const source of ['appImage', 'systemPackage'] as const) {
      const t = trashCopy({ displayName: 'A', source });
      for (const k of ['title', 'body', 'confirm', 'cancel'] as const)
        check(t[k], `trash ${source}.${k}`);
    }
  });
  // FINDING: Failed-with-no-preview (inspect failure) passes app '' -> sentence with a hole.
  it('inspect failure (no preview, app "") reads as a full sentence', () => {
    for (const c of ERROR_CODES) {
      const out = text(errorCopy(c, { app: '' }).message);
      expect(out, `code ${c} -> "${out}"`).not.toMatch(/ \.| {2}|\bwhile installing\./);
      expect(out, `code ${c} -> "${out}"`).not.toMatch(/^\s|\s\./);
    }
  });
  // FINDING: busy has no action -> Alert without any button.
  it('every error code offers a button (no dead-end alert)', () => {
    for (const c of ERROR_CODES) {
      expect(errorCopy(c, { app: 'A' }).action, `code ${c} has no button`).not.toBeNull();
    }
  });
});
