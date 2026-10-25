# ADR-0003: Clean Architecture enforced by crate boundaries

- **Status:** Accepted
- **Date:** 2026-10-25

## Context
The interesting logic (compatibility, install orchestration, error mapping) must be testable without D-Bus, root or a GUI. We also expect to swap backends (PackageKit or dnf5daemon) and targets (Fedora, then Ubuntu).

## Decision
Use a Cargo workspace in `src-tauri` with crates `domain` → `application` (use cases + port traits) → `infrastructure` (adapters). The `installer` app crate is the composition root and the only one that depends on `tauri`. The frontend mirrors this with `core/domain`, `core/ports`, feature controllers, and `infrastructure/{tauri,mock}`.

## Consequences
- The compiler forbids illegal dependencies (for example, domain can't import `zbus`).
- Every use case can be unit-tested with fakes from `application::testing`.
- There is a little more boilerplate (DTO mapping, traits), which is accepted.

## Alternatives considered
A flat `src-tauri/src/*.rs` (handoff plan): faster at first, but the logic would leak into commands.
