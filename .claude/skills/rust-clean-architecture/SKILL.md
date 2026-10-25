---
name: rust-clean-architecture
description: Write Rust in src-tauri following the domain / application / infrastructure / shell crate split, error and async conventions, and the testing approach. Use for any Rust change.
---

# Rust conventions (Clean Architecture)

Read `docs/architecture/ARCHITECTURE.md` §4–8 and ADR-0003.

## Where code goes
| Crate | Contains | Allowed deps |
|---|---|---|
| `installer-domain` | entities, value objects, pure rules, domain errors | `serde`, `thiserror` |
| `installer-application` | use cases, port traits, `PreviewRegistry`, `testing` fakes | domain, `tracing`, `trait-variant`, `tokio-util` (CancellationToken only) |
| `installer-infrastructure` | adapters implementing ports | domain, application, `zbus`, `ashpd`, `gio`/`gtk`, `backhand`, `rpm`, `tokio`, `serde_json` |
| `installer` (src-tauri) | commands, DTOs, composition root | everything + `tauri` |

If you need IO in domain or application, you need a **port**. Define the trait in `application/src/ports`, implement it in infrastructure, and wire it in `src-tauri/src/composition.rs`.

## Style
- Edition 2024, `#![forbid(unsafe_code)]` in domain/application/infrastructure.
- Errors: `thiserror` enums per layer (`InspectError`, `BackendError`, `InstallError`). No `anyhow` outside `src-tauri/src/main.rs`. Never use `unwrap()`/`expect()` in non-test code, except for invariants proven locally (add a comment).
- Async: tokio. Use native `async fn` in traits plus `#[trait_variant::make(Send)]`. Store ports as `Arc<dyn …>` only where needed; otherwise use generics on use cases.
- Cancellation: pass `CancellationToken`, and `select!` on it in long adapter loops.
- Logging: `tracing` spans per use case (`#[tracing::instrument(skip(self, observer))]`). Never log file contents.
- Newtypes for IDs (`PreviewId`, `AppId`) and validated strings (`Slug`).
- Domain functions are pure and synchronous.

## Testing
- Domain: table tests plus `proptest` (version ordering, slugging, format sniffing).
- Application: each use case gets tests for success, cancel and every error branch, using `application::testing::{FakeSystemPackages, FakeUserApps, RecordingObserver, …}`.
- Infrastructure: parsers against `tests/fixtures/` (regenerate via `tests/fixtures/make-fixtures.sh`). D-Bus tests are `#[ignore]` and run with `pnpm test:integration` in a Fedora container.
- A test in `infrastructure/inspect` asserts that the module doesn't use `std::process`: no executing packages (ADR-0005).

## Verify
`cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
