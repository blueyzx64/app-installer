# Technical Plan — Tauri 2

> Verify specific config keys and D-Bus details against the current Tauri and PackageKit docs while building; APIs move.

## 1. Architecture

```
┌──────────────────────────────── Tauri app ────────────────────────────────┐
│  Front end (HTML/CSS/JS, from prototype/index.html)                       │
│   • DMG window, drag-to-Applications, sheets, progress, success           │
│   • calls Rust commands with invoke(), listens for progress events         │
├───────────────────────────────────────────────────────────────────────────┤
│  Rust back end                                                            │
│   inspect.rs    → what is this file? name, version, icon, needs root?      │
│   compat.rs     → reads /etc/os-release; can this distro install it?       │
│   packagekit.rs → .rpm/.deb install via PackageKit D-Bus (zbus)           │
│   appimage.rs   → copy, chmod, extract icon + .desktop, register          │
│   defaults.rs   → is default? make default (xdg-mime)                     │
└───────────────┬───────────────────────────────┬───────────────────────────┘
                │ D-Bus (system bus)            │ files in $HOME
        ┌───────▼────────┐              ┌───────▼─────────────────────────┐
        │  PackageKit    │──polkit──▶ system password dialog            │
        │  dnf  |  apt   │              │ ~/Applications, ~/.local/share │
        └────────────────┘              └─────────────────────────────────┘
```

## 2. Front end

- Start from `prototype/index.html`. It is already plain HTML, CSS and JavaScript, so it can be split into `index.html`, `styles.css` and `app.js`. You can add a framework (Svelte, React, Vue) later if the UI grows, but none is needed.
- Replace the simulated timers with real calls:
  - `invoke('inspect_package', { path })` when the window opens
  - `invoke('install', { path })` when the icon is dropped on Applications
  - `listen('install-progress', …)` for percentage and status updates
  - `listen('install-finished', …)` for success or a friendly error
- **Password step:** remove the custom password dialog. When a .rpm/.deb is dropped, show a dimmed window with "Waiting for permission…" while polkit's system dialog is open.
- Map back-end states to plain words, for example dependency resolution → "Getting a few things it needs".
- Window: use `decorations: false` with custom traffic-light controls (`data-tauri-drag-region` on the title bar), or keep the system decorations if the custom ones feel out of place on GNOME/KDE.

## 3. Back end

### 3.1 Opening a file
- The `.desktop` entry uses `Exec=installer %f`, so the file path arrives as a command-line argument.
- Use `tauri-plugin-single-instance` so a second double-click forwards the path to the window that's already running.

### 3.2 Inspecting a package
- **.rpm / .deb:** PackageKit `GetDetailsLocal` returns the name, summary and size without installing. As a fallback, read the headers directly (for example with the `rpm` crate, or `ar` + control file for .deb).
- **AppImage:** read the embedded `.desktop` file and `.DirIcon`. Note that `--appimage-extract` runs the AppImage's own runtime, so prefer reading the SquashFS image from Rust (for example with the `backhand` crate) so that nothing from an untrusted file is executed.

### 3.3 Distro compatibility
- Read `ID` and `ID_LIKE` from `/etc/os-release`.
- `.rpm` is OK on fedora / rhel / centos / opensuse-like systems; `.deb` is OK on debian / ubuntu-like systems.
- On a mismatch, show: "This app was made for Ubuntu-based computers, so it can't be installed here." If the developer offers one, link to an AppImage or Flatpak version.

### 3.4 Installing .rpm / .deb with PackageKit
Using `zbus` on the **system bus**:
1. `org.freedesktop.PackageKit` → `CreateTransaction()` returns a transaction object path.
2. On `org.freedesktop.PackageKit.Transaction`:
   - optionally run `InstallFiles` with the *simulate* flag first, to find out what dependencies it will pull in
   - `InstallFiles(transaction_flags, [path])`
3. Watch the `Percentage` and `Status` properties, plus the `ErrorCode` and `Finished` signals, and forward them to the front end as events.
4. PackageKit triggers the polkit password prompt itself. Treat a *not authorized* error as "cancelled" and quietly return the icon.

This works on Fedora (dnf back end) and on Ubuntu (apt back end) with no distro-specific code.

### 3.5 Installing AppImages (no root)
1. Copy to `~/Applications/<Name>.AppImage` and set mode `0755`.
2. Write the icon to `~/.local/share/icons/hicolor/<size>/apps/`.
3. Write `~/.local/share/applications/<name>.desktop`, with `Exec=` pointing at the copied file.
4. Run `update-desktop-database ~/.local/share/applications` if it is available.
5. Uninstalling reverses these steps.

### 3.6 Becoming the default installer
- In the app's `.desktop` file:
  `MimeType=application/x-rpm;application/x-redhat-package-manager;application/vnd.debian.binary-package;application/vnd.appimage;application/x-iso9660-appimage;`
- In Tauri, declare these with `bundle.fileAssociations`. Check that the generated `.desktop` file contains the `MimeType` line; if not, provide a custom desktop template for the deb/rpm bundles.
- "Make Default" runs `xdg-mime default <app-id>.desktop <type>` for each type.
- "Is default?" checks `xdg-mime query default <type>`.

## 4. Packaging

- `tauri.conf.json` → `bundle.targets: ["rpm", "deb", "appimage"]`
- `cargo tauri build` writes the outputs to `src-tauri/target/release/bundle/{rpm,deb,appimage}/`.
- Runtime dependencies are WebKitGTK 4.1 and PackageKit:
  - Fedora: `webkit2gtk4.1`, `PackageKit`
  - Ubuntu 22.04+: `libwebkit2gtk-4.1-0`, `packagekit`
- Add PackageKit to each bundle's dependency list in the config so it is pulled in automatically.

## 5. CI and distribution

- GitHub Actions: build on Ubuntu 22.04, the oldest glibc you support, so the binaries run on newer systems. Smoke-test install in Fedora and Ubuntu containers.
- Attach the .rpm, .deb and AppImage to each GitHub Release.
- Later: publish to a **Copr** repo (Fedora) and a **PPA** (Ubuntu) so the installer updates through the system's normal updates.
- Sign the packages before public release.

## 6. Suggested milestones

1. **Skeleton:** `npm create tauri-app`, drop in the prototype UI, open a file passed on the command line.
2. **AppImage path end-to-end:** no root needed, so it is the quickest way to prove the flow.
3. **PackageKit path:** .rpm on Fedora first, then .deb on Ubuntu; progress events; cancelled-password handling.
4. **Default-installer prompt** plus `xdg-mime` integration.
5. **Edge cases:** wrong-distro package, already installed, newer version installed, offline, broken file.
6. **Packaging and CI**, then test on fresh Fedora and Ubuntu VMs.
7. **Polish:** uninstall flow, accessibility pass, translations.

## 7. Suggested project layout

```
installer/
├─ src/                      # front end (from prototype/index.html)
│  ├─ index.html
│  ├─ styles.css
│  └─ app.js
└─ src-tauri/
   ├─ tauri.conf.json
   ├─ Cargo.toml             # tauri, zbus, serde, tokio, dirs
   └─ src/
      ├─ main.rs             # commands + single-instance + events
      ├─ inspect.rs
      ├─ compat.rs
      ├─ packagekit.rs
      ├─ appimage.rs
      └─ defaults.rs
```
