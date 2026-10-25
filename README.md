# app-installer

A friendly installer for Linux that works like the Mac one. You double-click a downloaded app, then drag its icon onto the **Applications** folder. That's it. No terminal, no commands.

## Why

Installing a downloaded app on Linux usually means a terminal or a software store that is slow to open. This app does one job: open the file you downloaded, tell you what it is, and install it with one drag. If the app needs permission, your system asks for it in its own window. We never see your password.

## What it handles

- Apps downloaded as `.rpm` and AppImage files. `.deb` files come later.
- Fedora Workstation 45 with GNOME 51 first (Fedora 44 and GNOME 50 are the minimum).
- A small library window to open or remove the apps it installed.

## Status

Work in progress. The window, the install flow, the library and the AppImage path are built. Installing system packages (`.rpm`) is not connected yet, so for now the app uses a stand-in that says installing isn't available. Everything runs with pretend data in a browser.

## Try it (pretend mode, no Linux setup needed)

You need Node 22 and pnpm.

```bash
pnpm install
pnpm dev          # open http://localhost:1420
```

Pick a different story with `?scenario=` in the address, for example `http://localhost:1420/?scenario=appimage`. The full list is in [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md#mock-scenarios).

To build the real app you also need Rust and some system libraries. See the [development guide](docs/DEVELOPMENT.md).

## Stack

Tauri 2, Rust, SvelteKit (Svelte 5), TypeScript, PackageKit over D-Bus.

## Documentation

| | |
|---|---|
| [Architecture map](docs/architecture/ARCHITECTURE.md) | Layers, ports and adapters, flows, testing |
| [Folder structure](docs/architecture/folder-structure.md) | Where every kind of file goes (as built) |
| [IPC contract](docs/architecture/ipc-contract.md) · [State machines](docs/architecture/state-machines.md) | Frontend and Rust API, UI behaviour |
| [Features](docs/product/FEATURES.md) · [Roadmap](docs/product/ROADMAP.md) | What we build, in what order |
| [Design system](docs/design/DESIGN-SYSTEM.md) · [Copy](docs/design/COPY.md) | Look and plain-language text |
| [Fedora / GNOME notes](docs/platform/fedora-gnome.md) | PackageKit, polkit, portals, AppImage details |
| [Decisions (ADRs)](docs/adr/README.md) | Why things are the way they are |
| [Development guide](docs/DEVELOPMENT.md) | Setup and commands |
| [Prototype](design/prototype/index.html) | Open in a browser to try the intended flow |
| [Original handoff](docs/handoff/README.md) | Historical context |

AI-assisted development: see [CLAUDE.md](CLAUDE.md), `.claude/skills/`, and [start-workflow.md](start-workflow.md).
