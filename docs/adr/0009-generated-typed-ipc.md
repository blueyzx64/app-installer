# ADR-0009: Generated, typed IPC bindings

- **Status:** Proposed (confirm in M0 spike 3)
- **Date:** 2026-10-25

## Decision
Generate `src/lib/infrastructure/tauri/bindings.ts` from Rust DTOs and commands with `tauri-specta` (+ `specta`). Commit the file, and have CI regenerate it and fail on any difference. If `tauri-specta` isn't stable enough for Channels, use `ts-rs` for types and hand-written `invoke` wrappers instead.

## Consequences
Rust and TS can't drift apart silently. Every DTO derives `specta::Type`.
