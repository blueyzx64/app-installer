// Proves the dependency-rule lint (CLAUDE.md non-negotiable 1) fires. Run: node --test scripts/
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { ESLint } from 'eslint';

const eslint = new ESLint({ cwd: process.cwd() });

async function hits(filePath, code) {
  const [r] = await eslint.lintText(code, { filePath });
  return r.messages.filter((m) => m.ruleId === 'no-restricted-imports').length;
}

const tauri = "import { invoke } from '@tauri-apps/api/core';\ninvoke;\n";
const svelte = "import { onMount } from 'svelte';\nonMount;\n";
const kit = "import { goto } from '$app/navigation';\ngoto;\n";

test('tauri import outside infrastructure/tauri is rejected', async () => {
  assert.equal(await hits('src/lib/features/x/a.ts', tauri), 1);
  assert.equal(await hits('src/routes/a.ts', tauri), 1);
  assert.equal(await hits('src/lib/infrastructure/mock/a.ts', tauri), 1);
});

test('tauri import inside infrastructure/tauri is allowed', async () => {
  assert.equal(await hits('src/lib/infrastructure/tauri/gateway.ts', tauri), 0);
});

test('core may not import svelte, sveltekit or tauri', async () => {
  assert.equal(await hits('src/lib/core/a.ts', tauri), 1);
  assert.equal(await hits('src/lib/core/a.ts', svelte), 1);
  assert.equal(await hits('src/lib/core/a.ts', kit), 1);
  assert.equal(await hits('src/lib/core/deep/b.svelte.ts', svelte), 1);
});

test('svelte is fine outside core', async () => {
  assert.equal(await hits('src/lib/ui/a.ts', svelte), 0);
});
