---
name: tauri-ipc
description: Add or change a Tauri command, Channel, event, plugin or capability end-to-end (Rust use case → command → DTO → generated TS bindings → gateway → mock). Use whenever the frontend/backend contract changes.
---

# Tauri 2 IPC end-to-end

Contract doc: `docs/architecture/ipc-contract.md` (update it in the same change).

## Steps to add a command
1. **Use case first**: add or extend it in `src-tauri/crates/application/src/use_cases/`, with tests that use fakes.
2. **DTOs** in `src-tauri/src/dto.rs`: `#[derive(Serialize, Deserialize, specta::Type)]`, `#[serde(rename_all = "camelCase")]`, and tagged enums with `#[serde(tag = "kind", rename_all = "camelCase")]`. Add `From<domain::X> for XDto`. DTOs never leak into the application crate.
3. **Command** in `src-tauri/src/commands/<area>.rs`:
   ```rust
   #[tauri::command]
   #[specta::specta]
   pub async fn open_package(state: State<'_, AppState>, path: String) -> Result<PackagePreviewDto, CommandError> {
       let path = crate::cli::validate_package_path(&path)?;
       Ok(state.open_package.execute(path).await?.into())
   }
   ```
   Keep the body to about 10 lines with no logic. Errors become `CommandError { code, detail }`.
4. **Register** it in the specta builder in `lib.rs` (`collect_commands![...]`). The same builder provides `invoke_handler`.
5. **Regenerate** with `pnpm bindings`, and commit `src/lib/infrastructure/tauri/bindings.ts`.
6. **Gateway**: add it to the port, then to the tauri gateway and the mock gateway (see the `svelte5-frontend` skill).
7. **Capability**: custom commands are allowed by default for app commands. If you use core or plugin APIs from JS (window, event), add the *narrowest* permission to `src-tauri/capabilities/<window>.json` with a comment.

## Streaming progress
- Use `tauri::ipc::Channel<T>` as a command argument (`on_event: Channel<InstallEventDto>`). Wrap it in an `InstallObserver` implementation in `events.rs`, and `channel.send(...)` from the observer. Ignore send errors (the window may have closed) but log them at debug level.
- Use `app.emit("app://open-file", payload)` only for global notifications.

## Single instance and argv
- Register `tauri_plugin_single_instance::init(|app, argv, _cwd| …)` **first**. Parse argv in `cli.rs`, then emit `app://open-file` and focus the main window.
- The initial argv is stored in `AppState.launch_file` and taken once via `take_launch_file`.

## Security rules
- No `shell`, `fs` or `http` plugins. The frontend reaches the system only through our commands.
- Validate every path argument: canonicalise it, make sure it is a regular file and at most 8 GiB, and never follow it into `/proc` or `/dev`.
- CSP in `tauri.conf.json`: `default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; font-src 'self'`.

## Verify
`cargo test --workspace`, `pnpm bindings && git diff --exit-code src/lib/infrastructure/tauri/bindings.ts`, `pnpm check`, `pnpm test`.
