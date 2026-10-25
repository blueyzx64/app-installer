# Architecture Decision Records

One decision per file, numbered, never deleted. To change a decision, add a new ADR that supersedes the old one and set the old one's status to `Superseded by ADR-XXXX`. Copy [`template.md`](template.md).

| ADR | Title | Status |
|---|---|---|
| [0001](0001-tauri-2.md) | Tauri 2 as the application shell | Accepted |
| [0002](0002-sveltekit-spa.md) | SvelteKit in SPA mode with Svelte 5 runes | Accepted |
| [0003](0003-clean-architecture-crates.md) | Clean Architecture enforced by crate boundaries | Accepted |
| [0004](0004-packagekit-primary-dnf5daemon-fallback.md) | PackageKit primary, dnf5daemon fallback | Proposed (M0 spike 2) |
| [0005](0005-inspect-appimages-without-executing.md) | Inspect AppImages without executing them | Accepted |
| [0006](0006-window-controls-follow-gnome-layout.md) | Mac-style window controls placed by GNOME button-layout | Accepted (corner/shadow: M0 spike 1) |
| [0007](0007-polkit-owns-authentication.md) | polkit owns authentication | Accepted |
| [0008](0008-fedora-first.md) | Fedora first, Ubuntu later | Accepted |
| [0009](0009-generated-typed-ipc.md) | Generated, typed IPC bindings | Accepted (spike 3 confirmed) |
