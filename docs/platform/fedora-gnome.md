# Platform Notes: Fedora 45 / GNOME 51 (minimum Fedora 44 / GNOME 50)

> Facts checked in October 2026. Re-verify anything marked ⚠ before relying on it.

## Versions

| | Fedora 44 (Apr 2026) | Fedora 45 (Oct 2026) |
|---|---|---|
| GNOME | 50 | **51 "A Coruña"** (Fedora 45 ships GNOME 51, not 50) |
| GTK | 4.22 | 4.24 |
| Session | Wayland only (GNOME 50 removed the X11 session; XWayland remains for X11 apps) | Wayland only |
| PackageKit backend | **dnf5** (`PackageKit-backend-dnf5`, default since F44; dnf4 backend dropped) | dnf5 |
| GNOME Software | Moved toward a downstream dnf5daemon plugin; a revert to PackageKit was discussed ⚠ | ⚠ verify in M0 spike 2 |
| WebKitGTK for Tauri | `webkit2gtk4.1` (GTK3 API, soup3) | same ⚠ confirm still packaged |

What this means for us:
- **Wayland-only:** no global window positioning; `startDragging` and the drag region must go through the compositor (tao handles this). There are no X11 workarounds.
- **PackageKit is still the portable API**, and on Fedora it now drives libdnf5. If it isn't installed by default on Workstation 45, our RPM's `Requires: PackageKit` pulls it in. The dnf5daemon adapter (`org.rpm.dnf.v0`) is the fallback ([ADR-0004](../adr/0004-packagekit-primary-dnf5daemon-fallback.md)).
- GNOME 51 added a **Reduced Motion** setting and the `ext-background-effect-v1` blur protocol. The webview can't use the protocol, so CSS `backdrop-filter` is our only blur.

## Dev machine setup (Fedora)

```bash
sudo dnf install webkit2gtk4.1-devel openssl-devel curl wget file \
  libappindicator-gtk3-devel librsvg2-devel libxdo-devel \
  gcc gcc-c++ rpm-build PackageKit appstream desktop-file-utils
sudo dnf group install "c-development"     # dnf5 syntax
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
corepack enable && corepack prepare pnpm@latest --activate   # Node LTS from dnf or fnm
cargo install tauri-cli --locked            # or use `pnpm tauri` from @tauri-apps/cli
```

(⚠ Package list follows Tauri's prerequisites page; check it again when scaffolding.)

## D-Bus services

| Service | Bus | Name / path | Used for |
|---|---|---|---|
| PackageKit | system | `org.freedesktop.PackageKit` at `/org/freedesktop/PackageKit` | `CreateTransaction` → `org.freedesktop.PackageKit.Transaction` (`InstallFiles`, `GetDetailsLocal`, `Resolve`, `RemovePackages`, `Cancel`, `SetHints`) |
| dnf5daemon | system | `org.rpm.dnf.v0` (`SessionManager.open_session` → per-session objects; `Goal.do_transaction` needs polkit `auth_admin`) | Fallback |
| polkit | system | `org.freedesktop.PolicyKit1` | Not called by us directly; PackageKit/dnf5daemon call it |
| Settings portal | session | `org.freedesktop.portal.Desktop` → `org.freedesktop.portal.Settings` | `org.freedesktop.appearance`: `color-scheme`, `accent-color`, `contrast` (use the `ashpd` crate) |

polkit actions to expect: `org.freedesktop.packagekit.package-install`, `org.freedesktop.packagekit.package-install-untrusted` (local unsigned files), `org.freedesktop.packagekit.package-remove`.

PackageKit details worth knowing (⚠ check `pk-enum.h`):
- `Percentage` = 101 means unknown.
- `Status` values include `WAIT`, `SETUP`, `RUNNING`, `QUERY`, `DOWNLOAD`, `INSTALL`, `CLEANUP`, `FINISHED` and **`WAITING_FOR_AUTH`**. The last one is how we know to show the permission shade.
- Transaction flags: `ONLY_TRUSTED`, `SIMULATE`, `ONLY_DOWNLOAD`, `ALLOW_REINSTALL`, `JUST_REINSTALL`, `ALLOW_DOWNGRADE` (a bitfield).
- Errors to map: `NOT_AUTHORIZED`, `TRANSACTION_CANCELLED`, `NO_NETWORK`, `DEP_RESOLUTION_FAILED`, `FILE_CONFLICTS`, `NO_SPACE_ON_DEVICE`, `MISSING_GPG_SIGNATURE`/`BAD_GPG_SIGNATURE`, `INVALID_PACKAGE_FILE`, `PACKAGE_ALREADY_INSTALLED`, `INCOMPATIBLE_ARCHITECTURE`.

## GSettings we read

| Schema / key | Why | How |
|---|---|---|
| `org.gnome.desktop.wm.preferences button-layout` | Window control placement | `gtk::Settings::default().gtk_decoration_layout()` (GDK Wayland maps it) with a `notify` signal; fallback `gio::Settings`. **Not** exposed by the portal (xdg-desktop-portal-gnome#95) |
| `org.gnome.desktop.interface enable-animations` | Reduced motion | `gio::Settings` or portal namespace `org.gnome.desktop.interface` |
| `org.gnome.desktop.interface text-scaling-factor` | Text scaling | WebKitGTK usually follows it; verify |

## Freedesktop files we write

| File | Content |
|---|---|
| `~/Applications/<Name>.AppImage` | The copied AppImage, mode 0755 (`~/Applications` is created if missing) |
| `~/.local/share/applications/<app-id>.<slug>.desktop` | A generated entry; validate with `desktop-file-validate` in tests |
| `~/.local/share/icons/hicolor/{48x48,128x128,256x256,scalable}/apps/<app-id>.<slug>.{png,svg}` | Icons |
| `~/.config/mimeapps.list` | Written by GIO when the user chooses Make Default |
| `$XDG_DATA_HOME/<app-id>/ledger.json` | Our record of what we installed |
| `$XDG_STATE_HOME/<app-id>/log/` | Logs |

MIME types: `application/x-rpm` (alias `application/x-redhat-package-manager`), `application/vnd.appimage`, `application/x-iso9660-appimage`, and later `application/vnd.debian.binary-package` (alias `application/x-deb`).

## AppImage specifics

- Type 2 = an ELF runtime followed by SquashFS. Magic `AI\x02` at byte offset 8. SquashFS offset = `e_shoff + e_shentsize * e_shnum` from the ELF header.
- Older runtimes need `libfuse.so.2` (`fuse-libs` on Fedora). Newer static runtimes don't. Check `libfuse.so.2` via ld cache paths, and treat a static runtime as OK.
- Some AppImages use DwarFS instead of SquashFS: report them as unsupported in v1.
- SELinux: files in `~/Applications` get `user_home_t`, and executing them is allowed for unconfined users (the default). No relabel is needed.

## Ubuntu notes (for later)

- Ubuntu 24.04+ restricts unprivileged user namespaces through AppArmor, so many Electron AppImages fail to start without `--no-sandbox`. Expect support questions; consider detecting it.
- `packagekit` and `libwebkit2gtk-4.1-0` are available; the apt backend handles `.deb` through the same D-Bus API.
