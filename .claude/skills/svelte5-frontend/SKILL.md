---
name: svelte5-frontend
description: Write SvelteKit + Svelte 5 + TypeScript code for this app using runes, feature controllers, ports/gateways and the mock/Tauri split. Use when creating routes, components, controllers or gateways under src/.
---

# Svelte 5 frontend conventions

Architecture: `docs/architecture/ARCHITECTURE.md` §10 and `folder-structure.md`.

## Layers (imports only point downward)
`routes` → `features/*` (components + `*Controller.svelte.ts`) → `core/ports` + `core/domain` ← `infrastructure/{tauri,mock}` (implements ports).
- `core/**` must not import `svelte`, `$app/*` or `@tauri-apps/*`.
- Only `infrastructure/tauri/**` may import `@tauri-apps/api`.
- Components never call gateways. They call controller methods and read controller state.

## Runes
- State lives in classes in `.svelte.ts` files: `class InstallerController { state = $state<InstallerState>(...); percent = $derived(...) }`.
- Use `$props()` with a typed `interface Props`, `$bindable` only for genuine two-way bindings, and `$derived` rather than `$effect` for computed values. Use `$effect` only to sync with the outside world (DOM, timers, subscriptions), and always return a cleanup.
- Event handlers are attributes (`onclick`, `onpointerdown`), not `on:`. Use snippets (`{#snippet}` / `{@render}`), not slots.
- Don't use legacy `svelte/store` in new code (except where a library requires it).

## Controllers
- Hold a pure reducer state: `this.state = installerMachine(this.state, event)`. Logic lives in `core/domain/*-machine.ts` with vitest tests.
- Get gateways via `useGateways()` from `$lib/app/context` (set in `+layout.svelte`).
- Subscriptions (events, channels) are started in a method and returned as an unsubscribe function; the component calls it in an `$effect` cleanup.

## Adding a gateway method
1. Add it to the port interface in `core/ports/*.ts`.
2. Implement it in `infrastructure/tauri/*-gateway.ts` (using the generated `bindings.ts`) **and** in `infrastructure/mock/*-gateway.ts` (timer-driven, with fixtures).
3. Unit-test the mock behaviour, and run e2e against the mock.

## Routing
- `src/routes/+layout.ts`: `export const ssr = false;`. No `+page.server.ts` or `+server.ts`, ever (there is no server).
- Use `goto()` from `$app/navigation` for view switches. The window size is changed through `WindowGateway`.

## Testing
- `vitest` for domain and controllers (with mock gateways), `@testing-library/svelte` for components, Playwright (`e2e/`) on `pnpm dev`.
- Drag tests: dispatch `pointerdown`/`pointermove`/`pointerup` with coordinates; don't use HTML5 DnD.

## Commands
`pnpm check && pnpm lint && pnpm test` must pass.
