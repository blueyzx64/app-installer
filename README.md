# app-installer

A friendly, macOS-style installer for Linux. Double-click a downloaded app, then drag its icon onto **Applications**. That's it.

- **Formats:** `.rpm` and AppImage (v1), `.deb` later
- **Platform:** Fedora Workstation 45 / GNOME 51 first (minimum Fedora 44 / GNOME 50)
- **Stack:** Tauri 2 · Rust · SvelteKit (Svelte 5) · TypeScript · PackageKit over D-Bus

> Status: **planning**. The architecture and guides are in place; the code is scaffolded in milestone M0.

## Documentation

| | |
|---|---|
| [Architecture map](docs/architecture/ARCHITECTURE.md) | Clean Architecture layers, ports and adapters, flows, testing |
| [Folder structure](docs/architecture/folder-structure.md) | Where every kind of file goes |
| [IPC contract](docs/architecture/ipc-contract.md) · [State machines](docs/architecture/state-machines.md) | Frontend ⇄ Rust API, UI behaviour |
| [Features](docs/product/FEATURES.md) · [Roadmap](docs/product/ROADMAP.md) | What we build, in what order |
| [Design system](docs/design/DESIGN-SYSTEM.md) · [Copy](docs/design/COPY.md) | AppKit-inspired UI that respects GNOME; plain-language text |
| [Fedora / GNOME notes](docs/platform/fedora-gnome.md) | PackageKit, polkit, portals, AppImage details |
| [Decisions (ADRs)](docs/adr/README.md) | Why things are the way they are |
| [Development guide](docs/DEVELOPMENT.md) | Setup and commands |
| [Prototype](design/prototype/index.html) | Open in a browser to try the intended flow |
| [Original handoff](docs/handoff/README.md) | Historical context |

AI-assisted development: see [CLAUDE.md](CLAUDE.md) and `.claude/skills/`.
