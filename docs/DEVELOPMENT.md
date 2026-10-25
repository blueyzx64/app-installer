# Development Guide

## Prerequisites

Fedora 44+ (GNOME). See [platform/fedora-gnome.md](platform/fedora-gnome.md#dev-machine-setup-fedora) for the packages to install. You need Rust stable (via rustup), Node 22 and pnpm 10 (see `packageManager` in `package.json`).

## Ubuntu / Debian containers (CI and agent sandboxes)

CI runs on Ubuntu 24.04. These packages were enough to compile the Tauri shell there:

```bash
sudo apt-get install -y --no-install-recommends build-essential pkg-config \
  libwebkit2gtk-4.1-dev libjavascriptcoregtk-4.1-dev libsoup-3.0-dev \
  libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libssl-dev libdbus-1-dev
```

There is no system or session D-Bus in a plain container. Tests for D-Bus adapters use pure mapping functions and fake proxies. The real-bus tests are `#[ignore]` and need a Fedora machine or container.

## Commands

| Task | Command |
|---|---|
| Install JS deps | `pnpm install` |
| UI in a browser with mock gateways | `pnpm dev` then http://localhost:1420 |
| Production build of the UI | `pnpm build` (preview with `pnpm preview`) |
| Full app | `pnpm tauri dev` |
| Open a file in dev | `pnpm tauri dev -- -- ~/Downloads/foo.AppImage` |
| Type check | `pnpm check` (svelte-kit sync, then svelte-check) |
| Lint | `pnpm lint` (prettier --check, then eslint) · `pnpm format` fixes formatting |
| Banned-word check on UI text | `pnpm lint:copy` |
| Frontend tests | `pnpm test` (vitest, then the Node tests in `scripts/`) |
| Rust checks | `cd src-tauri && cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace` |
| Regenerate TS bindings | `pnpm bindings` (runs `cargo run --bin export-bindings`; CI fails if `bindings.ts` differs) |
| Release build | `pnpm tauri build` |

`pnpm e2e` and `pnpm test:integration` are planned and do not exist yet.

CI (`.github/workflows/ci.yml`) runs three jobs: frontend (`check`, `lint`, `lint:copy`, `test`, `build`), Rust (fmt, clippy, test) and a bindings drift check.

### Build directory tip

The Rust build is large. When several checkouts share one machine, point them at one build directory so it is compiled once:

```bash
export CARGO_TARGET_DIR=$HOME/.cache/installer-target
```

### Mock scenarios

In `pnpm dev` the mock gateways read `?scenario=<name>` from the address. The default is `rpm`.

| Scenario | Shows |
|---|---|
| `rpm` | a normal system package |
| `deb` | a `.deb` package |
| `appimage` | an AppImage (no password step) |
| `incompatible-arch` | a package built for another processor |
| `already-installed` | the same version is already installed |
| `update` | a newer version than the installed one |
| `fail-offline` | install fails without a network |
| `cancel-permission` | the permission prompt is dismissed (a cancel, not an error) |
| `missing-fuse2` | an AppImage that needs a missing system part |
| `empty-library` | the library with no apps |

The "Open a Package…" button will also honour `?pick=cancel`, which acts as if the file chooser was dismissed (contracts v2, not wired yet).

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
