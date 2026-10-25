# Development Guide

## Prerequisites

Fedora 44+ (GNOME). See [platform/fedora-gnome.md](platform/fedora-gnome.md#dev-machine-setup-fedora) for the packages to install. You need Rust stable (via rustup), Node LTS and pnpm.

## Commands (once scaffolded in M0)

| Task | Command |
|---|---|
| Install JS deps | `pnpm install` |
| UI in a browser with mock gateways | `pnpm dev` → http://localhost:1420 |
| Full app | `pnpm tauri dev` |
| Open a file in dev | `pnpm tauri dev -- -- ~/Downloads/foo.AppImage` |
| Frontend checks | `pnpm check` (svelte-check) · `pnpm lint` · `pnpm test` (vitest) · `pnpm e2e` (Playwright) |
| Rust checks | `cd src-tauri && cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace` |
| Regenerate TS bindings | `pnpm bindings` (runs the specta export; CI fails if `bindings.ts` differs) |
| D-Bus integration tests | `pnpm test:integration` (starts a Fedora 45 podman container with PackageKit, then runs `cargo test -- --ignored`) |
| Release build | `pnpm tauri build` → `src-tauri/target/release/bundle/{rpm,appimage}/` |

## Before every PR

1. All of the checks above pass locally.
2. New behaviour has tests at the lowest layer that can hold it (domain before application before infrastructure before UI).
3. User-facing text is in `en.json` and follows [design/COPY.md](design/COPY.md).
4. If you changed the IPC, update `ipc-contract.md`, the bindings and the mock gateway.
5. If you made an architectural choice, add an ADR in `docs/adr/`.

## Commit style

Conventional Commits with feature IDs: `feat(installer): F-24 treat dismissed polkit as cancel`. Scopes: `installer`, `library`, `defaults`, `appearance`, `ui`, `domain`, `app`, `infra`, `shell`, `build`, `docs`.

## Troubleshooting

- Blank window or flicker on NVIDIA: try `WEBKIT_DISABLE_DMABUF_RENDERER=1 pnpm tauri dev`.
- polkit dialog doesn't appear in a VM: make sure you are in a real GNOME session (not over SSH) and that `polkit` and `gnome-shell` are running.
- `org.freedesktop.PackageKit` not activatable: `sudo dnf install PackageKit` and check that `systemctl status packagekit` shows it activates on demand.
