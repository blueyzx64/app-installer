# State Machines

The UI's behaviour lives in pure reducers in `src/lib/core/domain/*-machine.ts`. They are unit-tested exhaustively and contain no timers, IO or Svelte code. Controllers (`*.svelte.ts`) keep the state in `$state` and translate gestures and gateway callbacks into events.

## Installer window

```mermaid
stateDiagram-v2
    [*] --> Inspecting: OPEN(path)
    Inspecting --> Ready: INSPECTED(preview, compat=ok)
    Inspecting --> Incompatible: INSPECTED(compat≠ok)
    Inspecting --> Failed: INSPECT_FAILED(code)

    Ready --> Dragging: DRAG_START
    Dragging --> Ready: DRAG_CANCEL / drop outside (spring back)
    Dragging --> Starting: DROP_ON_FOLDER
    Ready --> Starting: KEY_INSTALL (Enter/Space on icon)

    Starting --> AwaitingPermission: PROGRESS(waitingForPermission)
    Starting --> Installing: PROGRESS(other)
    AwaitingPermission --> Installing: PROGRESS(downloading|installing)
    AwaitingPermission --> Ready: CANCELLED (icon springs back, no error)
    Installing --> Succeeded: FINISHED(installed)
    Installing --> Ready: FINISHED(cancelled)
    Installing --> Failed: FINISHED(failed)

    Failed --> Ready: RETRY
    Succeeded --> [*]: OPEN_APP / CLOSE
    Incompatible --> [*]: CLOSE

    Ready --> Inspecting: OPEN(other path)
```

`Incompatible` sub-kinds: `alreadyInstalled.same` shows "Already installed" with an **Open** button; `alreadyInstalled.older` shows "A newer version is already installed"; `alreadyInstalled.newer` is not incompatible at all, it means **Update**: the state stays `Ready` and the folder label becomes "Update".

### State shape

```ts
type InstallerState =
  | { tag: 'inspecting'; path: string }
  | { tag: 'ready'; preview: PackagePreview; queued: string[] }
  | { tag: 'dragging'; preview: PackagePreview; overFolder: boolean }
  | { tag: 'starting' | 'awaitingPermission'; preview: PackagePreview }
  | { tag: 'installing'; preview: PackagePreview; phase: InstallPhase; percent: number | null; etaSecs: number | null; plan?: Plan }
  | { tag: 'succeeded'; preview: PackagePreview; app: InstalledApp }
  | { tag: 'incompatible'; preview: PackagePreview }
  | { tag: 'failed'; preview: PackagePreview | null; code: ErrorCode; detail: string | null };
```

Invariants (test them):
- You can't leave `starting`, `awaitingPermission` or `installing` through user gestures except **Cancel** (when `AllowCancel` is true).
- `OPEN` while installing appends to `queued`. It never replaces the preview.
- Progress `percent` never goes backwards within a phase. The reducer clamps it. PackageKit reports 101 for "unknown", which becomes `null` and the bar turns indeterminate.

## First-launch default prompt

```mermaid
stateDiagram-v2
    [*] --> Checking
    Checking --> Hidden: isDefault || askedBefore
    Checking --> Prompting: !isDefault && !askedBefore
    Prompting --> Applying: MAKE_DEFAULT
    Prompting --> Hidden: NOT_NOW (persist askedBefore)
    Applying --> Confirmed: ok → toast "Installer will open app packages"
    Applying --> Hidden: error (silent, logged)
    Confirmed --> Hidden: after 4s
```

The sheet is shown **after** the package is inspected, and never on top of the permission shade or an active install.

## Library (installed apps)

```mermaid
stateDiagram-v2
    [*] --> Loading
    Loading --> Empty: list = []
    Loading --> Listing: list ≠ []
    Listing --> Confirming: TRASH(app)
    Confirming --> Listing: CANCEL
    Confirming --> Removing: CONFIRM
    Removing --> Listing: removed (tile animates out) / cancelled
    Removing --> Listing: failed (toast with plain message)
```
