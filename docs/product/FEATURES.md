# Feature Set

Priority key: **MVP** = must work for the first public build · **v1** = in the 1.0 release · **v1.x** = soon after · **Later** = parked, with the idea recorded.
Each feature has an ID. Reference it in commits and PRs (e.g. `feat(installer): F-03 spring-back on cancel`).

## A. Opening packages

| ID | Feature | Priority | Acceptance criteria |
|---|---|---|---|
| F-01 | Open a package passed on the command line (`Exec=installer %f`) | MVP | Double-clicking `.rpm`/`.AppImage` in Files opens the installer window with name, version and icon within 1 s |
| F-02 | Single instance: a second open forwards to the running app | MVP | No second process stays alive. The new file is shown, or queued if an install is running |
| F-03 | Detect the format by content (magic bytes), not by extension | MVP | A renamed `.AppImage` without an extension still works. A `.rpm` that is really HTML shows "This file is damaged or isn't an app package" |
| F-04 | Inspect AppImages without executing them | MVP | Inspection makes no `exec`/`spawn` calls (asserted in tests). Name and icon come from the embedded `.desktop` and `.DirIcon` |
| F-05 | Inspect .rpm headers (name, version, summary, arch) | MVP | Works offline and without PackageKit |
| F-06 | Open by dropping a file onto the installer or library window | v1 | Same result as F-01 |
| F-07 | "Open a Package…" button (file chooser filtered to supported types) | v1 | Uses the GTK/portal file chooser. Nothing is typed into a text field |
| F-08 | .rpm icon extraction from the payload (hicolor or AppStream) | v1.x | A real icon for most Fedora/third-party RPMs; generic icon as the fallback |
| F-09 | .deb inspection | v1.x (Ubuntu target) | `control` fields parsed |

## B. Compatibility and safety checks (before the drag)

| ID | Feature | Priority | Acceptance criteria |
|---|---|---|---|
| F-10 | Wrong distro family | MVP | `.deb` on Fedora: "This app was made for Ubuntu-based computers, so it can't be installed here." No Applications folder shown |
| F-11 | Wrong architecture | MVP | `aarch64` RPM on `x86_64` gives a plain explanation |
| F-12 | Already installed: same version | MVP | "Lumen Notes is already installed" with an **Open** button |
| F-13 | Newer version on disk shows "Update" | v1 | Folder label becomes "Update"; the flow is otherwise identical |
| F-14 | Older version than installed | v1 | "A newer version is already installed." No downgrade in v1 |
| F-15 | AppImage needs FUSE 2 but it is missing | v1 | Plain explanation and a link to help. Static-runtime AppImages are not flagged |
| F-16 | Unsigned or untrusted source notice | Later (open question) | Design needed; PackageKit already escalates to the `package-install-untrusted` polkit action |

## C. Installing

| ID | Feature | Priority | Acceptance criteria |
|---|---|---|---|
| F-20 | Drag icon onto Applications to install | MVP | Pointer drag with mouse and touch; the folder highlights on hover; the icon "flies in" on drop; dropping elsewhere springs back |
| F-21 | Keyboard install (Enter/Space on the focused icon) | MVP | Announced via `aria-describedby`: "Press Enter to install" |
| F-22 | AppImage install to `~/Applications` with menu entry and icon | MVP | Appears in GNOME's app grid within 2 s; launches; no password asked |
| F-23 | .rpm install via PackageKit with the system polkit dialog | MVP | Window dims to "Waiting for permission…"; GNOME's auth dialog appears; on success the app is in the app grid |
| F-24 | Cancelled password is not an error | MVP | Dismissing polkit springs the icon back to its place, with no red text |
| F-25 | Plain-language progress with time estimate | MVP | Phases map to COPY.md phrases; indeterminate when the percentage is unknown; never shows package names of dependencies |
| F-26 | Friendly failures with optional "Details" | MVP | Each `ErrorCode` has a sentence and an action (Try Again / Close). The technical detail is hidden in a disclosure |
| F-27 | Dependency preview (simulate) | v1 | If extra downloads exceed 50 MB, the progress view says "Downloading a few things it needs (120 MB)". No confirmation step |
| F-28 | Cancel during download | v1 | A Cancel button shows only while PackageKit `AllowCancel` is true |
| F-29 | Success screen with **Open** and **Done** | MVP | **Open** launches via the desktop file; the window closes after launch |
| F-30 | .deb install via PackageKit (apt backend) | v1.x (Ubuntu) | Same flow as F-23 |
| F-31 | dnf5daemon fallback backend | v1 (decide in M0) | When PackageKit can't be activated, .rpm installs still work |

## D. Installed apps library and uninstall (v1 scope, chosen)

| ID | Feature | Priority | Acceptance criteria |
|---|---|---|---|
| F-40 | Library window: grid of apps installed **through this installer** | v1 | Opens when the app is launched without a file. Shows an empty state with "Open a Package…" |
| F-41 | Move AppImage to Trash | v1 | Confirmation alert; moves to the freedesktop Trash (restorable in Files); menu entry and icons removed |
| F-42 | Remove system package | v1 | Confirmation alert ("You'll be asked for your password"); polkit; if other packages would be removed too, the alert says "This will also remove 2 other items that only this app used" |
| F-43 | Reconcile the ledger with reality | v1 | Apps removed outside the installer disappear from the library on the next open |
| F-44 | Drag a tile out to the Trash zone | Later | The macOS gesture; needs design |
| F-45 | Show apps installed by other means | Later | Out of scope; GNOME Software does this |

## E. System integration

| ID | Feature | Priority | Acceptance criteria |
|---|---|---|---|
| F-50 | Offer to become the default app for packages (once) | MVP | Sheet: **Make Default** / **Not Now**; remembered; the toast confirms |
| F-51 | MIME registration in the shipped `.desktop` | MVP | `application/x-rpm; application/vnd.appimage; application/x-iso9660-appimage;` (+ `application/vnd.debian.binary-package` later) |
| F-52 | AppStream metainfo for GNOME Software | v1 | Validates with `appstreamcli validate` |
| F-53 | Follow GNOME dark/light, accent colour, high contrast, reduced motion live | MVP | Changing them in GNOME Settings updates the window without a restart |
| F-54 | Window controls follow GNOME `button-layout` (macOS-style circles) | MVP | The default `appmenu:close` shows a single close on the right; `close,minimize,maximize:` shows left-side traffic lights |
| F-55 | Respect text scaling (`text-scaling-factor`) | v1 | Layout holds at 1.25× |
| F-56 | Translations | v1.x | Strings already externalised in MVP |

## F. Quality and operations

| ID | Feature | Priority | Acceptance criteria |
|---|---|---|---|
| F-60 | Logs to `$XDG_STATE_HOME/<app-id>/log` | MVP | Rotated; no personal data |
| F-61 | Browser-mock mode for UI work | MVP | `pnpm dev` in Chromium/Firefox runs every flow with fixtures |
| F-62 | RPM release build plus AppImage | MVP | CI artefacts on tags |
| F-63 | Copr repository | v1 | `dnf copr enable …` → updates through GNOME Software |
| F-64 | Signed RPMs | v1 | |

## Parked / later ideas

- Flatpak bundles (`.flatpakref`, `.flatpak`), handed to the Flatpak system helper or `flatpak` portal
- AppImage updates (zsync / AppImageUpdate)
- Multiple simultaneous installer windows (like several mounted DMGs)
- KDE Plasma polish (it already works through PackageKit and polkit-kde)
