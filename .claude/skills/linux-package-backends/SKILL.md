---
name: linux-package-backends
description: Implement or debug package inspection and installation on Linux — PackageKit over D-Bus (zbus), the dnf5daemon fallback, AppImage SquashFS reading and user installs, .desktop entries, icons, MIME default handler, and polkit behaviour. Use for anything in crates/infrastructure touching packages or the desktop.
---

# Linux package backends and desktop integration

References: `docs/platform/fedora-gnome.md`, ADR-0004, ADR-0005, ADR-0007, ARCHITECTURE §7 and §9.

## PackageKit adapter (`infrastructure/src/packagekit/`)
- Use a `zbus` **system** connection, shared and created lazily. Generate proxies with `#[zbus::proxy]` for `org.freedesktop.PackageKit` (default path `/org/freedesktop/PackageKit`) and `org.freedesktop.PackageKit.Transaction`.
- Use **one transaction per operation**: `CreateTransaction()` → `SetHints(["interactive=true", "locale=<LANG>", "background=false"])` → the method.
- Inspect a local file: `GetDetailsLocal([path])` (optional; the `rpm` crate is the primary inspector).
- Plan: `InstallFiles(SIMULATE, [path])` and collect `Package` signals for the count and size.
- Install: `InstallFiles(flags, [path])`. Try `ONLY_TRUSTED` first. On `MISSING_GPG_SIGNATURE` / `NOT_AUTHORIZED` for trust reasons, retry in a new transaction without it (this escalates to the `package-install-untrusted` polkit action). Keep the trust policy in one function, because F-16 will revisit it.
- Progress: subscribe to `PropertiesChanged` (`Status`, `Percentage`, `RemainingTime`, `AllowCancel`) **before** calling the method. Map `Status` to `InstallPhase` in `mapping.rs`: `WAITING_FOR_AUTH` → `WaitingForPermission`; `DOWNLOAD*` → `Downloading`; `INSTALL`/`RUNNING`/`COMMIT` → `Installing`; `CLEANUP`/`FINISHED` → `Finishing`; everything else → `Preparing`. `Percentage == 101` → `None`.
- Completion: wait for the `Finished(exit, runtime)` signal. Collect `ErrorCode(code, details)` before it. `NOT_AUTHORIZED` or exit `CANCELLED` → `InstallOutcome::Cancelled` (not an error!).
- Cancel: `Cancel()` only when `AllowCancel` is true.
- Already installed: `Resolve(filter=installed, [name])`.
- Remove: `RemovePackages(flags, [package_id], allow_deps=false, autoremove=true)`. Simulate first to count extra removals.
- ⚠ Check enum values against `pk-enum.h` for the installed PackageKit version and keep them in `enums.rs` with doc links.

## dnf5daemon adapter (feature `dnf5daemon`)
- `org.rpm.dnf.v0.SessionManager.open_session({})` → session object → add the local RPM to the goal → `Goal.resolve` → `Goal.do_transaction` (polkit `auth_admin`) → `close_session`. ⚠ Confirm the method names against `dnf5daemon-dbus-api(8)` on the target Fedora.
- Map its progress signals to the same `InstallPhase` values.

## AppImage inspection (no execution)
1. Read the first 64 bytes: ELF magic, plus `AI\x02` at offset 8 → type 2. `AI\x01` → type 1 → `Unsupported`.
2. Compute the SquashFS offset as `e_shoff + e_shentsize * e_shnum` (handle 32- and 64-bit and endianness), and check the `hsqs` magic there. Anything else (for example DwarFS) → `Unsupported`.
3. Use `backhand::FilesystemReader::from_reader_with_offset`. Read the root `*.desktop` (`freedesktop-desktop-entry` crate) for `Name`, `Comment`, `Icon` and `X-AppImage-Version`; read `.DirIcon` or `<Icon>.png|svg`, resolving symlinks only inside the image and capping each file at 2 MiB.
4. Runtime check: the runtime is dynamic and `libfuse.so.2` isn't present → `Compatibility::MissingRuntime("fuse2")`.

## AppImage install / uninstall
- Copy to `~/Applications/.<slug>.partial` (with 1 MiB buffer and progress), `fsync`, then `rename` to `<DisplayName>.AppImage`. If that exists and isn't ours → suffix ` (2)`. Then `set_permissions(0o755)`.
- Icons go to `~/.local/share/icons/hicolor/<size>/apps/<app-id>.<slug>.png` (resize with the `image` crate to 48/128/256) or `scalable/…svg`.
- Desktop entry: **generate** it with `Type=Application`, `Name`, `Comment`, `Exec="<abs path>" %U`, `TryExec=<abs path>`, `Icon=<app-id>.<slug>`, `Categories` (sanitised from the source), `Terminal=false`, `X-Installer-Managed=true`, `X-Installer-Source=<abs path>`. Escape per the Desktop Entry spec. Then run `update-desktop-database ~/.local/share/applications` only if it exists (it is fine to ignore a failure).
- Uninstall: `gio::File::for_path(p).trash()`, remove the desktop file and icons, then `ledger.forget`.

## Default handler
- `gio::DesktopAppInfo::new("<app-id>.desktop")` → `set_as_default_for_type(mime)` for each MIME type in `defaults.rs::SUPPORTED_MIME`.
- `is_default` = every type's `AppInfo::default_for_type(mime, false)` id equals ours.

## Never
- Execute a package to inspect it. Use `sudo` or `pkexec`. Shell out to `dnf`, `rpm` or `apt`. Write outside `$HOME` ourselves.
