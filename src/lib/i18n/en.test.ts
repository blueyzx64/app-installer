import { describe, expect, it } from 'vitest';
import en from './en.json';

// Banned words (COPY.md). They may only appear inside the collapsed Details disclosure, which is
// rendered from raw error text, never from en.json.
const BANNED = [
  'rpm',
  'deb',
  'appimage',
  'package manager',
  'dnf',
  'apt',
  'packagekit',
  'polkit',
  'root',
  'sudo',
  'dependency',
  'dependencies',
  'repository',
  'transaction',
  'path',
  'terminal',
  'mime',
  'binary'
];

describe('en.json', () => {
  it('has COPY.md contract keys', () => {
    for (const k of [
      'dmg.hint',
      'dmg.folder',
      'dmg.folderUpdate',
      'dmg.needsPassword',
      'dmg.keyboardHint',
      'perm.waiting',
      'perm.sub',
      'phase.preparing',
      'phase.downloading',
      'phase.downloadingSize',
      'phase.installing',
      'phase.finishing',
      'eta.seconds',
      'eta.minute',
      'eta.minutes',
      'eta.unknown',
      'done.title',
      'done.body',
      'done.open',
      'done.done',
      'default.title',
      'default.body',
      'default.make',
      'default.notNow',
      'default.toast',
      'library.title',
      'library.empty',
      'library.open',
      'trash.title.user',
      'trash.body.user',
      'trash.title.system',
      'trash.body.system',
      'trash.body.extras',
      'trash.confirm.user',
      'trash.confirm.system',
      'trash.cancel'
    ]) {
      expect(en, k).toHaveProperty([k]);
    }
  });
  it('uses no banned words', () => {
    for (const [k, v] of Object.entries(en)) {
      const words: string[] = v.toLowerCase().match(/[a-z]+(?: manager)?/g) ?? [];
      for (const w of BANNED) {
        expect(words.includes(w), `${k}: "${v}" contains "${w}"`).toBe(false);
      }
    }
  });
  it('uses typographic ellipsis, not three dots', () => {
    for (const [k, v] of Object.entries(en)) expect(v, k).not.toContain('...');
  });
});
