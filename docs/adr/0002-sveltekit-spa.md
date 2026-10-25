# ADR-0002: SvelteKit in SPA mode with Svelte 5 runes

- **Status:** Accepted
- **Date:** 2026-10-25

## Context
We have two or three views (installer, library, and possibly settings later), and we want typed, reactive UI code with very little runtime overhead.

## Decision
SvelteKit with `@sveltejs/adapter-static` (`fallback: 'index.html'`), `ssr = false` in the root `+layout.ts`, Svelte 5 runes (`$state`, `$derived`, `$effect`, `$props`), and TypeScript in strict mode. This follows Tauri's documented SvelteKit setup.

## Consequences
- File-based routing for `/install` and `/library`.
- No server: load functions run in the webview only, so never use `+page.server.ts` or `+server.ts`.
- Stores are replaced by classes in `.svelte.ts` files that hold `$state`.

## Alternatives considered
Plain Svelte + Vite (simpler, but no routing), React (heavier, and the team asked for Svelte).
