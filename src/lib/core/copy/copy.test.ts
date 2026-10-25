import { describe, expect, it } from 'vitest';
import { ERROR_CODES, type Compatibility, type InstallPhase } from '../domain/types';
import { messages, t } from '../../i18n';
import {
  compatCopy,
  errorCopy,
  etaCopy,
  phaseCopy,
  removeErrorCopy,
  text,
  trashCopy
} from './copy';

const app = 'Lumen Notes';
const phases: InstallPhase[] = [
  'preparing',
  'waitingForPermission',
  'downloading',
  'installing',
  'finishing'
];
const compats: Compatibility[] = [
  { kind: 'ok' },
  { kind: 'wrongFamily', madeFor: 'debBased' },
  { kind: 'wrongFamily', madeFor: 'rpmBased' },
  { kind: 'wrongArch', packageArch: 'a', hostArch: 'b' },
  { kind: 'alreadyInstalled', relation: 'same', installedVersion: '1' },
  { kind: 'alreadyInstalled', relation: 'older', installedVersion: '1' },
  { kind: 'alreadyInstalled', relation: 'newer', installedVersion: '1' },
  { kind: 'missingRuntime', what: 'fuse2' },
  { kind: 'unsupported', reason: 'appImageType1' },
  { kind: 'unsupported', reason: 'dwarfs' },
  { kind: 'unsupported', reason: 'unknownFormat' }
];

describe('phaseCopy', () => {
  it('maps every phase to an existing key', () => {
    for (const p of phases) expect(phaseCopy(p, { app }).key in messages).toBe(true);
  });
  it('texts', () => {
    expect(text(phaseCopy('preparing', { app }))).toBe('Getting ready…');
    expect(text(phaseCopy('waitingForPermission', { app }))).toBe('Waiting for permission…');
    expect(text(phaseCopy('installing', { app }))).toBe('Installing Lumen Notes…');
    expect(text(phaseCopy('finishing', { app }))).toBe('Finishing up…');
  });
  it('downloading mentions size only above 50 MB', () => {
    const small = { extraPackages: 1, downloadBytes: 50_000_000 };
    const big = { extraPackages: 1, downloadBytes: 62_000_000 };
    expect(text(phaseCopy('downloading', { app }))).toBe('Getting a few things it needs');
    expect(text(phaseCopy('downloading', { app, plan: small }))).toBe(
      'Getting a few things it needs'
    );
    expect(text(phaseCopy('downloading', { app, plan: big }))).toBe(
      'Getting a few things it needs (62 MB)'
    );
  });
});

describe('etaCopy', () => {
  it('phrases', () => {
    expect(text(etaCopy(null))).toBe('');
    expect(text(etaCopy(10))).toBe('A few seconds left');
    expect(text(etaCopy(60))).toBe('About a minute left');
    expect(text(etaCopy(180))).toBe('About 3 minutes left');
  });
});

describe('compatCopy', () => {
  it('is null only for installable packages', () => {
    const nulls = compats.filter((c) => compatCopy(c, app) === null).map((c) => JSON.stringify(c));
    expect(nulls).toEqual([
      JSON.stringify({ kind: 'ok' }),
      JSON.stringify({ kind: 'alreadyInstalled', relation: 'newer', installedVersion: '1' })
    ]);
  });
  it('every key exists and the app name is filled in', () => {
    for (const c of compats) {
      const copy = compatCopy(c, app);
      if (!copy) continue;
      expect(copy.title.key in messages).toBe(true);
      if (copy.body) expect(copy.body.key in messages).toBe(true);
      expect(text(copy.title)).not.toContain('{');
    }
  });
  it('same version offers Open', () => {
    const c = compatCopy(
      { kind: 'alreadyInstalled', relation: 'same', installedVersion: '1' },
      app
    );
    expect(text(c!.title)).toBe('Lumen Notes is already installed');
    expect(c!.action).toBe('open');
    expect(c!.body).toBeNull();
  });
  it('wrongFamily bodies differ by family', () => {
    const deb = compatCopy({ kind: 'wrongFamily', madeFor: 'debBased' }, app)!;
    const rpm = compatCopy({ kind: 'wrongFamily', madeFor: 'rpmBased' }, app)!;
    expect(text(deb.body!)).toContain('Ubuntu-based');
    expect(text(rpm.body!)).toContain('Fedora-based');
  });
});

describe('errorCopy', () => {
  it('covers every ErrorCode with existing keys and an action', () => {
    for (const code of ERROR_CODES) {
      const c = errorCopy(code, { app });
      expect(c.message.key in messages).toBe(true);
      expect(text(c.message)).not.toContain('{');
    }
  });
  it('actions per COPY.md', () => {
    const action = (c: (typeof ERROR_CODES)[number]) => errorCopy(c, { app }).action;
    expect(action('offline')).toBe('tryAgain');
    expect(action('unknown')).toBe('tryAgain');
    expect(action('notEnoughSpace')).toBe('tryAgain');
    expect(action('permissionDenied')).toBe('close');
    expect(action('corruptFile')).toBe('close');
    expect(action('busy')).toBeNull();
  });
  it('notEnoughSpace uses the size when known', () => {
    expect(text(errorCopy('notEnoughSpace', { app, spaceBytes: 120_000_000 }).message)).toBe(
      'There isn’t enough space. Free up 120 MB and try again.'
    );
    expect(text(errorCopy('notEnoughSpace', { app }).message)).not.toContain('{');
  });
  it('names the app, never a file', () => {
    expect(text(errorCopy('offline', { app }).message)).toContain('Lumen Notes');
  });
});

describe('removeErrorCopy and trashCopy', () => {
  it('every code resolves', () => {
    for (const code of ERROR_CODES) {
      expect(text(removeErrorCopy(code, app))).not.toContain('{');
    }
  });
  it('user app goes to the Trash, system app asks for a password', () => {
    const u = trashCopy({ displayName: app, source: 'appImage' });
    const s = trashCopy({ displayName: app, source: 'systemPackage' });
    expect(text(u.title)).toBe('Move “Lumen Notes” to the Trash?');
    expect(text(u.confirm)).toBe('Move to Trash');
    expect(u.system).toBe(false);
    expect(text(s.title)).toBe('Remove “Lumen Notes”?');
    expect(text(s.body)).toBe('You’ll be asked for your password.');
    expect(s.system).toBe(true);
  });
});

describe('i18n t()', () => {
  it('fills params and leaves unknown params', () => {
    expect(t('done.title', { app })).toBe('Lumen Notes is installed');
    expect(t('done.title')).toBe('{app} is installed');
  });
  it('unknown key returns the key', () => {
    expect(t('nope.nope' as never)).toBe('nope.nope');
  });
  it('numbers work', () => {
    expect(t('eta.minutes', { n: 4 })).toBe('About 4 minutes left');
  });
});
