import js from '@eslint/js';
import svelte from 'eslint-plugin-svelte';
import globals from 'globals';
import ts from 'typescript-eslint';

const tauriPattern = {
  group: ['@tauri-apps/*'],
  message: 'Only src/lib/infrastructure/tauri may import @tauri-apps/* (CLAUDE.md rule 1).'
};
const corePatterns = [
  tauriPattern,
  {
    group: ['svelte', 'svelte/*', '@sveltejs/*', '$app/*', '$env/*', '*.svelte'],
    message: 'src/lib/core must not import Svelte or SvelteKit (CLAUDE.md rule 1).'
  }
];

export default ts.config(
  {
    ignores: [
      'build/',
      '.svelte-kit/',
      'node_modules/',
      'src-tauri/',
      'design/',
      'docs/',
      '.claude/',
      'src/lib/infrastructure/tauri/bindings.ts'
    ]
  },
  js.configs.recommended,
  ...ts.configs.recommended,
  ...svelte.configs['flat/recommended'],
  {
    languageOptions: { globals: { ...globals.browser, ...globals.node } }
  },
  // Dependency rule (CLAUDE.md non-negotiable 1). Later blocks replace earlier ones.
  {
    files: ['src/**/*.{ts,js,svelte}'],
    rules: { 'no-restricted-imports': ['error', { patterns: [tauriPattern] }] }
  },
  {
    files: ['src/lib/core/**'],
    rules: { 'no-restricted-imports': ['error', { patterns: corePatterns }] }
  },
  {
    files: ['src/lib/infrastructure/tauri/**'],
    rules: { 'no-restricted-imports': 'off' }
  },
  {
    files: ['**/*.svelte', '**/*.svelte.ts'],
    languageOptions: { parserOptions: { parser: ts.parser } }
  }
);
