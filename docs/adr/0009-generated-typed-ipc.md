# ADR-0009: Generated, typed IPC bindings

- **Status:** Proposed (confirm in M0 spike 3)
- **Date:** 2026-10-25

## Decision
Generate `src/lib/infrastructure/tauri/bindings.ts` from Rust DTOs and commands with `tauri-specta` (+ `specta`). Commit the file, and have CI regenerate it and fail on any difference. If `tauri-specta` isn't stable enough for Channels, use `ts-rs` for types and hand-written `invoke` wrappers instead.

## Consequences
Rust and TS can't drift apart silently. Every DTO derives `specta::Type`.

## Spike result (M0 spike 3, 2026-10-25, lane D)

**Works. `tauri-specta` is usable; no fallback to `ts-rs` needed.**

- Versions (pinned with `=`): `tauri-specta 2.0.0-rc.26` (features `derive`, `typescript`), `specta 2.0.0-rc.26`, `specta-typescript 0.0.13`, `tauri 2.12`. `specta 2.0.0-rc.27` and `specta-typescript 0.0.14` do not resolve together with `tauri-specta rc.26`; keep all three on the rc.26 / 0.0.13 line.
- Proven: a plain command (`take_launch_file`) and a command with a `tauri::ipc::Channel<InstallEventDto>` argument (`install`) both generate typed wrappers in `bindings.ts`. Tagged enums (`#[serde(tag = "kind")]`) and camelCase fields come out as expected.
- Generate with `pnpm bindings` (runs `cargo run --bin export-bindings`, which calls `installer_lib::bindings::export`). A unit test in `bindings.rs` checks the export contains the commands and the Channel signature. CI drift check: `pnpm bindings && git diff --exit-code src/lib/infrastructure/tauri/bindings.ts`.
- Gotchas:
  - `u64`/`i64`/`usize` are refused by `specta-typescript` (precision). For byte counts use `#[specta(type = specta_typescript::Number)]` on the field (safe below 2^53; packages are capped at 8 GiB), or a smaller integer type.
  - Types used with serde in both directions are emitted as `X`, `X_Serialize` and `X_Deserialize`. Commands return the `_Serialize` shape (`detail: string | null`). A `Channel<T>` argument is typed with the `_Deserialize` shape (`percent?: number | null`), which is slightly looser than what Rust sends; gateways should treat such optional fields as `undefined | null`.
  - The generated command wrappers return `{ status, data | error }` (a `typedError` helper); gateways unwrap that.

### Update (W0b)

The builder now uses `.disable_serde_phases()` (one TS type per Rust type; no `_Serialize` / `_Deserialize` pairs, so the `Channel<T>` looseness above is gone) and `.error_handling(ErrorHandlingMode::Throw)` (command wrappers are plain `Promise<T>` and reject with `CommandError`). Void commands are typed `Promise<null>`.
