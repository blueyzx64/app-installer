---
name: release-packaging
description: Configure tauri.conf.json bundling, the custom .desktop template, AppStream metainfo, RPM dependencies, GitHub Actions CI/release, Copr and signing. Use when touching src-tauri/tauri.conf.json bundle settings, src-tauri/linux/, or .github/workflows.
---

# Release and packaging

## tauri.conf.json (bundle)
```jsonc
{
  "identifier": "io.github.blueyzx64.Installer",      // placeholder until the product is named
  "bundle": {
    "active": true,
    "targets": ["rpm", "appimage"],                    // add "deb" for the Ubuntu target
    "category": "Utility",
    "shortDescription": "Install apps by dragging them to Applications",
    "fileAssociations": [
      { "ext": ["rpm"], "mimeType": "application/x-rpm", "name": "App package" },
      { "ext": ["AppImage", "appimage"], "mimeType": "application/vnd.appimage", "name": "App" }
    ],
    "linux": {
      "rpm": { "depends": ["PackageKit", "webkit2gtk4.1"], "desktopTemplate": "linux/installer.desktop" },
      "deb": { "depends": ["packagekit", "libwebkit2gtk-4.1-0"], "desktopTemplate": "linux/installer.desktop" }
    }
  }
}
```
⚠ Check the key names against the current Tauri config schema (`https://schema.tauri.app/config/2`). After building, run `rpm -qlp` and `rpm -qRp` on the output, and check that the installed `.desktop` contains our `MimeType=` and `Exec=… %f`.

## src-tauri/linux/installer.desktop
```
[Desktop Entry]
Type=Application
Name=Installer
Comment=Install apps by dragging them to Applications
Exec={{exec}} %f
Icon={{icon}}
Terminal=false
Categories=System;PackageManager;
MimeType=application/x-rpm;application/x-redhat-package-manager;application/vnd.appimage;application/x-iso9660-appimage;
StartupNotify=true
```
Validate it with `desktop-file-validate`. Ship `<identifier>.metainfo.xml` and validate it with `appstreamcli validate --pedantic`.

## CI (.github/workflows)
- `ci.yml` (PRs): job `frontend` (pnpm install, check, lint, lint:copy, test, build). Job `rust` in `fedora:44` (dnf install the deps; then fmt, clippy -D warnings, test). Job `bindings` (regenerate, then `git diff --exit-code`).
- `release.yml` (tags `v*`): build in `fedora:44` (the oldest supported, for glibc and WebKitGTK), upload `.rpm` and `.AppImage` to the GitHub Release, then smoke-test `dnf install ./*.rpm` in `fedora:45`.
- Cache `~/.cargo`, `src-tauri/target` and the pnpm store.

## Signing and distribution
- Sign the RPM with a project GPG key (`rpmsign --addsign`) in CI from a secret. Publish the public key.
- Copr: an `.spec` that wraps the Tauri build, or upload the SRPM. Enable it with `dnf copr enable <owner>/installer`.
- Later: PPA for Ubuntu.

## Versioning
SemVer. The single source is `src-tauri/Cargo.toml` `version`; `tauri.conf.json` reads it (omit `version` there), and `package.json` is kept in sync by `pnpm version:sync`.
