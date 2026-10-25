# Decision Log

Status key: **Decided** = agreed in conversation · **Proposed** = recommended, not yet confirmed

## Product

| # | Decision | Why | Status |
|---|---|---|---|
| P1 | Build a macOS DMG-style installer: drag the app icon onto an Applications folder | Familiar and simple; needs no understanding of package managers | Decided |
| P2 | Support **.rpm, .deb and AppImage** | Covers Fedora, Ubuntu/Mint and distro-independent apps | Decided |
| P3 | **Drag only**, with no Install button | Keeps the experience identical to a DMG; one obvious action | Decided |
| P4 | Keyboard Enter on the focused app icon also installs | Accessibility fallback for people who can't drag; invisible to everyone else | Proposed |
| P5 | Dropping a **.deb/.rpm** asks for permission and then the password; **AppImages need no password** | System packages install for all users and need root; AppImages install just for the current user | Decided |
| P6 | **Plain language everywhere**: no paths, commands or package-manager names | Target users are non-technical | Decided |
| P7 | **macOS-inspired design**: traffic-light controls, sheets, centred auth dialog, spring animations | Requested look; familiar to many users | Decided |
| P8 | Offer to become the **default app for packages** on first launch ("Make Default" / "Not Now"), confirmed by a notification | Double-clicking any downloaded package should open the installer | Decided |
| P9 | Use the system's polkit dialog for the real password prompt, with the installer window dimmed behind it | Apps cannot securely collect admin passwords; the prototype's custom dialog is illustrative only | Proposed (platform constraint) |
| P10 | Detect a wrong-distro package (.deb on Fedora, .rpm on Ubuntu) and explain it kindly | Native installs across distro families aren't possible | Proposed |

## Technical

| # | Decision | Why | Status |
|---|---|---|---|
| T1 | **Framework: Tauri 2** (Rust + HTML/CSS/JS) | Prototype UI carries over almost directly; full freedom for the macOS look and animations; one tool builds .rpm, .deb and AppImage; small binaries (~10 MB) | **Decided** |
| T2 | Install .rpm/.deb through **PackageKit over D-Bus** (via the `zbus` crate) | One code path for dnf (Fedora) and apt (Ubuntu); handles dependencies; triggers polkit automatically | Proposed |
| T3 | Install AppImages by copying to `~/Applications`, setting the executable bit and writing a `.desktop` entry plus icon | No root needed; matches how Gear Lever and AppImageLauncher work | Proposed |
| T4 | Register as default via `.desktop` `MimeType=` and `xdg-mime default` | Standard freedesktop mechanism; works on GNOME and KDE | Proposed |
| T5 | **Primary output: .rpm** (Fedora). Also ship .deb and AppImage from the same build | Requested format; Tauri produces all three at no extra cost | Decided (.rpm), Proposed (others) |
| T6 | Build releases in GitHub Actions; later publish via **Copr** (Fedora) and a **PPA** (Ubuntu) | Automatic updates through each distro's normal update tool | Proposed |

## Alternatives considered for T1

- **Vala + GTK4:** the most native, lightest option and the closest fit to GNOME's own tools. Worth revisiting if WebKitGTK performance or native feel becomes a problem.
- **Qt/QML:** the best native option for custom animated UI.
- **Flutter, Electron:** rejected (weaker Linux support; too heavy).

## Open questions

1. **Product name and icon.** "Installer" is a placeholder.
2. **Uninstall.** Should dragging out of Applications, or a "Move to Trash" action, remove apps? This needs a design.
3. **AppImage updates.** Should we support AppImageUpdate / zsync, or leave updating to each app?
4. **Untrusted packages.** What warning, if any, should appear for unsigned .rpm/.deb files from the web?
5. **Flatpak bundles (`.flatpakref` / `.flatpak`).** Support them later?
6. **Minimum versions.** Fedora 40+ and Ubuntu 22.04+ are suggested, because Tauri 2 needs WebKitGTK 4.1.
7. **Behaviour when a package needs many dependencies.** Show the download size? Keep it hidden?
