// lint:copy — plain-language gate (CLAUDE.md non-negotiable 5).
//  1. src/lib/i18n/en.json values must not contain banned technical words (docs/design/COPY.md).
//  2. .svelte files must not hard-code user-visible text: text nodes and text attributes
//     (title, aria-label, alt, placeholder ...) must go through {t(...)}.
// Allow-list: scripts/lint-copy.allow.json (exact strings, e.g. the product name).
// Usage: node scripts/lint-copy.mjs [rootDir]
import { readFileSync, readdirSync, existsSync, statSync } from 'node:fs';
import { join, relative, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parse } from 'svelte/compiler';

const here = dirname(fileURLToPath(import.meta.url));
const root = process.argv[2] ?? join(here, '..');

// Words from docs/design/COPY.md "Banned words in the UI".
const BANNED = [
  'RPM',
  'DEB',
  'AppImage',
  'package manager',
  'dnf',
  'apt',
  'PackageKit',
  'polkit',
  'root',
  'sudo',
  'dependency',
  'dependencies',
  'repository',
  'transaction',
  'path',
  'terminal',
  'MIME',
  'binary'
];
// Format names are allowed in the "Details" disclosure (keys under `details`).
const DETAILS_OK = new Set(['rpm', 'deb', 'appimage']);
const TEXT_ATTRS = new Set([
  'title',
  'aria-label',
  'aria-description',
  'alt',
  'placeholder',
  'label'
]);

const bannedRe = BANNED.map((w) => ({
  word: w,
  re: new RegExp(`(?<![A-Za-z0-9])${w.replace(/ /g, '\\s+')}(?![A-Za-z0-9])`, 'i')
}));

const allowFile = join(here, 'lint-copy.allow.json');
const allow = existsSync(allowFile) ? JSON.parse(readFileSync(allowFile, 'utf8')) : { text: [] };
const allowText = new Set((allow.text ?? []).map((s) => s.trim()));

const problems = [];

function flatten(obj, prefix = '') {
  return Object.entries(obj).flatMap(([k, v]) => {
    const key = prefix ? `${prefix}.${k}` : k;
    return v && typeof v === 'object' ? flatten(v, key) : [[key, String(v)]];
  });
}

function lintEnJson(file) {
  let json;
  try {
    json = JSON.parse(readFileSync(file, 'utf8'));
  } catch (e) {
    problems.push(`${relative(root, file)}: invalid JSON (${e.message})`);
    return;
  }
  for (const [key, value] of flatten(json)) {
    const inDetails = key.split('.').includes('details');
    for (const { word, re } of bannedRe) {
      if (inDetails && DETAILS_OK.has(word.toLowerCase())) continue;
      if (re.test(value)) {
        problems.push(`${relative(root, file)}: "${key}" contains banned word "${word}": ${value}`);
      }
    }
  }
}

function isVisible(text) {
  const t = text.trim();
  if (!/\p{L}/u.test(t)) return false; // symbols, digits, punctuation only
  return !allowText.has(t);
}

function lintSvelte(file) {
  const src = readFileSync(file, 'utf8');
  let ast;
  try {
    ast = parse(src, { modern: true });
  } catch (e) {
    problems.push(`${relative(root, file)}: cannot parse (${e.message})`);
    return;
  }
  const report = (node, what, text) => {
    const line = src.slice(0, node.start).split('\n').length;
    problems.push(
      `${relative(root, file)}:${line}: hard-coded ${what} "${text.trim()}" — use {t(...)}`
    );
  };
  const walk = (node) => {
    if (!node || typeof node !== 'object') return;
    if (Array.isArray(node)) return node.forEach(walk);
    switch (node.type) {
      case 'Text':
        if (isVisible(node.data)) report(node, 'text', node.data);
        return;
      case 'Attribute': {
        if (TEXT_ATTRS.has(String(node.name).toLowerCase()) && Array.isArray(node.value)) {
          for (const part of node.value) {
            if (part.type === 'Text' && isVisible(part.data))
              report(part, `${node.name}=`, part.data);
          }
        }
        return; // other attributes (class, href ...) are not copy
      }
      case 'Comment':
      case 'StyleDirective': // style:width="{size}px" is CSS, not copy
        return;
      default:
        for (const [k, v] of Object.entries(node)) {
          if (k === 'type' || k === 'start' || k === 'end') continue;
          if (v && typeof v === 'object') walk(v);
        }
    }
  };
  walk(ast.fragment);
}

function* files(dir) {
  for (const name of readdirSync(dir)) {
    if (name === 'node_modules' || name.startsWith('.')) continue;
    const p = join(dir, name);
    if (statSync(p).isDirectory()) yield* files(p);
    else yield p;
  }
}

const enJson = join(root, 'src/lib/i18n/en.json');
if (existsSync(enJson)) lintEnJson(enJson);
else console.log('lint:copy: src/lib/i18n/en.json not found yet, skipping string check');

const srcDir = join(root, 'src');
let n = 0;
if (existsSync(srcDir)) {
  for (const f of files(srcDir)) {
    if (f.endsWith('.svelte')) {
      lintSvelte(f);
      n++;
    }
  }
}

if (problems.length) {
  console.error(problems.join('\n'));
  console.error(`\nlint:copy failed: ${problems.length} problem(s)`);
  process.exit(1);
}
console.log(`lint:copy ok (${n} svelte files)`);
