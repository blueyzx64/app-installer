// Proves lint:copy fires on fixtures in a temp dir. Run: node --test "scripts/*.test.mjs"
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';

const script = join(dirname(fileURLToPath(import.meta.url)), 'lint-copy.mjs');

function run(files) {
  const dir = mkdtempSync(join(tmpdir(), 'lint-copy-'));
  for (const [name, body] of Object.entries(files)) {
    const p = join(dir, name);
    mkdirSync(dirname(p), { recursive: true });
    writeFileSync(p, body);
  }
  return spawnSync('node', [script, dir], { encoding: 'utf8' });
}

test('clean tree passes', () => {
  const r = run({
    'src/lib/i18n/en.json': JSON.stringify({ dmg: { hint: 'Drag {app} to Applications' } }),
    'src/a.svelte':
      '<script lang="ts">let t = (k: string) => k;</script>\n<p class="x">{t("a")}</p>\n<span>→</span>'
  });
  assert.equal(r.status, 0, r.stderr);
});

test('banned word in en.json fails', () => {
  const r = run({ 'src/lib/i18n/en.json': JSON.stringify({ a: 'Open the terminal' }) });
  assert.equal(r.status, 1);
  assert.match(r.stderr, /terminal/);
});

test('format names allowed under details only', () => {
  const ok = run({ 'src/lib/i18n/en.json': JSON.stringify({ details: { format: 'RPM' } }) });
  assert.equal(ok.status, 0, ok.stderr);
  const bad = run({ 'src/lib/i18n/en.json': JSON.stringify({ a: 'This is an RPM' }) });
  assert.equal(bad.status, 1);
});

test('hard-coded svelte text fails', () => {
  const r = run({ 'src/a.svelte': '<button>Install now</button>' });
  assert.equal(r.status, 1);
  assert.match(r.stderr, /Install now/);
});

test('hard-coded aria-label fails', () => {
  const r = run({ 'src/a.svelte': '<button aria-label="Close window">x</button>' });
  assert.equal(r.status, 1);
});
