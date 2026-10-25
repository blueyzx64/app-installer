# CLAUDE.md

A macOS-style "drag to Applications" installer for Linux: Tauri 2 + SvelteKit (Svelte 5, TS) + Rust, Fedora 45 / GNOME 51 first.

## Read before working
- Architecture map: `docs/architecture/ARCHITECTURE.md` (layers, ports, flows)
- Where files go: `docs/architecture/folder-structure.md`
- IPC contract: `docs/architecture/ipc-contract.md` · UI state machines: `docs/architecture/state-machines.md`
- What to build: `docs/product/FEATURES.md` (feature IDs) · `docs/product/ROADMAP.md`
- Look and words: `docs/design/DESIGN-SYSTEM.md` · `docs/design/COPY.md`
- Platform facts: `docs/platform/fedora-gnome.md` · Decisions: `docs/adr/`

## Multi-agent builds
See `start-workflow.md` (roles, lanes, memory protocol, token rules).

## Skills (in .claude/skills, use the one that matches the task)
`apple-hig-design` · `svelte5-frontend` · `tauri-ipc` · `rust-clean-architecture` · `linux-package-backends` · `gnome-integration` · `plain-language-copy` · `release-packaging`

## Non-negotiables
1. **Dependency rule.** domain ← application ← infrastructure ← shell. Rust crates enforce it. In TS, only `src/lib/infrastructure/tauri` imports `@tauri-apps/*`, and `src/lib/core` imports neither Svelte nor Tauri.
2. **Never execute a package to inspect it** (ADR-0005). No `std::process` in `infrastructure/src/inspect`.
3. **Never collect passwords.** polkit owns auth (ADR-0007). A dismissed polkit dialog is a cancel, not an error.
4. **No shelling out** to dnf, rpm, apt, sudo or pkexec. Use PackageKit / dnf5daemon over D-Bus, and GIO for MIME, trash and launch.
5. **Plain language.** No technical words in the UI (list in COPY.md). Every string goes through `src/lib/i18n/en.json`.
6. **Every gateway has a mock.** `pnpm dev` in a plain browser must run every flow.
7. **IPC changes** update the Rust DTO, `bindings.ts` (generated), both gateways and `ipc-contract.md` together.
8. **Design tokens only** in components. Every animation has a reduced-motion variant. Window controls follow GNOME `button-layout`.

## Checks (must pass before commit)
- Frontend: `pnpm check && pnpm lint && pnpm test`
- Rust: `cd src-tauri && cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`

## Conventions
Conventional Commits with feature IDs (`feat(installer): F-20 …`). New architectural choices get an ADR. Facts marked ⚠ in the docs must be re-verified before you rely on them.
