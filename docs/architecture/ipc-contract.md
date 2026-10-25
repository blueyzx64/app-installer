# IPC Contract (frontend ⇄ Rust)

All commands are `async` Tauri commands that return `Result<T, CommandError>`. Types are written in Rust (`src-tauri/src/dto.rs`) and **generated** into TypeScript (`src/lib/infrastructure/tauri/bindings.ts`) by `tauri-specta`. The TS shapes below are illustrative; the generated file is authoritative.

Naming: commands use `snake_case` in Rust and camelCase wrappers in TS. Fields use `camelCase` on the wire (`#[serde(rename_all = "camelCase")]`). Enums use tags: `#[serde(tag = "kind")]`.

## Commands

| Command | Args | Returns | Notes |
|---|---|---|---|
| `take_launch_file` | none | `string \| null` | The path from argv on first launch. Consumed once |
| `open_package` | `{ path: string }` | `PackagePreview` | Parses only. Stores the preview server-side under `id` |
| `install` | `{ previewId: string, onEvent: Channel<InstallEvent> }` | `InstallOutcome` | Streams progress over the channel; resolves when finished |
| `cancel_install` | `{ previewId: string }` | `void` | Best effort (PackageKit `Cancel()` while `AllowCancel` is true) |
| `list_installed` | none | `InstalledApp[]` | Ledger reconciled with reality |
| `uninstall` | `{ appId: string, onEvent: Channel<InstallEvent> }` | `UninstallOutcome` | System packages prompt via polkit |
| `launch_app` | `{ appId: string }` | `void` | |
| `default_handler_status` | none | `DefaultStatus` | `{ isDefault: boolean, askedBefore: boolean }` |
| `make_default_handler` | none | `void` | For all supported MIME types |
| `dismiss_default_prompt` | none | `void` | Remembers "Not Now" |
| `get_appearance` | none | `Appearance` | Snapshot. Changes arrive through the `appearance://changed` event |

Window operations (close, minimize, start dragging) use `@tauri-apps/api/window` directly from `infrastructure/tauri/window-gateway.ts`, allowed through capabilities.

## Global events (Rust → all windows)

| Event | Payload | When |
|---|---|---|
| `app://open-file` | `{ path: string }` | Single-instance received a new file, or a file was dropped onto the window |
| `appearance://changed` | `Appearance` | Portal or GSettings change |

## Types (illustrative TS)

```ts
type PackageFormat = 'rpm' | 'deb' | 'appImage';
type InstallScope = 'system' | 'user';

interface PackagePreview {
  id: string;
  format: PackageFormat;
  displayName: string;
  version: string;
  summary: string | null;
  iconDataUrl: string | null;      // PNG/SVG as data: URL, <= 256 px
  sizeBytes: number;
  scope: InstallScope;
  needsPermission: boolean;        // scope === 'system'
  compat: Compatibility;
}

type Compatibility =
  | { kind: 'ok' }
  | { kind: 'wrongFamily'; madeFor: 'rpmBased' | 'debBased' }
  | { kind: 'wrongArch'; packageArch: string; hostArch: string }
  | { kind: 'alreadyInstalled'; relation: 'same' | 'older' | 'newer'; installedVersion: string }
  | { kind: 'missingRuntime'; what: 'fuse2' }
  | { kind: 'unsupported'; reason: 'appImageType1' | 'dwarfs' | 'unknownFormat' };

type InstallPhase = 'preparing' | 'waitingForPermission' | 'downloading' | 'installing' | 'finishing';

type InstallEvent =
  | { kind: 'plan'; extraPackages: number; downloadBytes: number }
  | { kind: 'progress'; phase: InstallPhase; percent: number | null; etaSecs: number | null };

type InstallOutcome =
  | { kind: 'installed'; app: InstalledApp }
  | { kind: 'cancelled' }
  | { kind: 'failed'; code: ErrorCode; detail: string | null };

interface InstalledApp {
  id: string;
  displayName: string;
  version: string;
  iconDataUrl: string | null;
  source: 'appImage' | 'systemPackage';
  installedAt: string;             // ISO-8601
  canLaunch: boolean;
}

type ErrorCode =
  | 'permissionDenied' | 'offline' | 'missingDependencies' | 'conflicts'
  | 'notEnoughSpace' | 'corruptFile' | 'untrustedSource' | 'backendUnavailable'
  | 'notFound' | 'busy' | 'unknown';

interface CommandError { code: ErrorCode; detail: string | null; }

interface Appearance {
  colorScheme: 'light' | 'dark';
  accent: string | null;           // '#RRGGBB' from the portal, null → default blue
  highContrast: boolean;
  reducedMotion: boolean;
  controls: { side: 'left' | 'right'; buttons: Array<'close' | 'minimize' | 'maximize'> };
}
```

## Rust side sketch

```rust
#[tauri::command]
#[specta::specta]
pub async fn install(
    state: tauri::State<'_, AppState>,
    preview_id: String,
    on_event: tauri::ipc::Channel<InstallEventDto>,
) -> Result<InstallOutcomeDto, CommandError> {
    let observer = ChannelObserver::new(on_event);
    let outcome = state.install_package.execute(preview_id.parse()?, &observer).await;
    Ok(outcome.into())
}
```

## Versioning rule

Any breaking change to a DTO needs all four of these in the **same** PR: the Rust DTO, the regenerated `bindings.ts`, both gateways (tauri + mock), and this document.
