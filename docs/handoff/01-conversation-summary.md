# Conversation Summary

## 1. The idea

**Question:** Can we make a Linux installer for .rpm and AppImage files that works like a macOS DMG, where you drag an app into Applications?

**Answer:** Yes, but the two formats are different under the hood.

- **AppImage** is already a self-contained app, much like a macOS `.app` bundle. Installing it only means copying it to `~/Applications`, making it executable, and adding a menu entry with its icon. No admin rights are needed.
- **RPM (and .deb)** packages are not self-contained. They need root, dependency resolution and an entry in the system package database. A drag-and-drop installer can only be a friendly front end that hands the file to the package manager.

Existing tools in this area include Gear Lever, AppImageLauncher and appimaged (for AppImages), and GNOME Software and KDE Discover (for packages).

## 2. First prototype

The scope grew to **.deb, .rpm and AppImage**, with a password prompt for system-level packages and macOS-style animations. A first interactive prototype was built with a package-type switcher, a DMG-style window, an Install button, a password sheet, a progress view that showed technical steps and the underlying command, and a success screen.

## 3. Second prototype (current)

Feedback and changes:

- **Drag only.** The Install button was removed. Dragging the icon onto Applications is the only way to install (Enter on the focused icon remains as an accessibility fallback).
- **Dropping a .deb or .rpm triggers the permission request**, which then asks for the password. AppImages install without one.
- **As non-technical as possible.** File paths, terminal commands and package-manager names were removed from the installer. Progress uses plain phrases such as "Getting a few things it needs" with a time estimate.
- **Default installer.** The prototype now starts on a desktop with the package in a Downloads window. Double-clicking it opens the installer, which asks once whether to become the default for app packages and confirms with a notification.
- **macOS design.** Traffic-light window controls, a translucent menu bar, a sheet that slides down from the title bar, a centred password dialog, and spring animations for the dragged icon.

The drag uses pointer events, so it works with a mouse and on touch screens.

## 4. Choosing tools

The first recommendation was **Tauri 2 + PackageKit**. When asked to justify it against Vala/GTK and others, the comparison was:

| Option | Strengths | Weaknesses |
|---|---|---|
| **Vala + GTK4** | Native, tiny, instant start-up; PackageKit's GLib library fits directly (elementary AppCenter uses it); native drag-and-drop | Small community; more manual packaging; a macOS look takes work against GNOME styling |
| **Qt / QML** | Best for custom animated designs while staying native; proven PackageKit bindings (KDE Discover) | Looks a little foreign on GNOME by default; LGPL licensing to review |
| **Tauri 2** | Fastest path from the prototype; full design freedom; builds .rpm, .deb and AppImage out of the box | Runs in WebKitGTK, so it can feel slightly less native; adds a WebKitGTK dependency; Rust learning curve |
| Flutter | `macos_ui` package copies macOS widgets | Linux desktop is a second-tier target |
| Electron | Familiar | Far too heavy for a small system utility |

**Final choice: Tauri 2.** See `02-decisions.md`.

## 5. Platform limits we must design around

1. **The password prompt belongs to the system.** Polkit shows its own dialog in the GNOME or KDE style. Apps cannot collect the password themselves, because that would let any app steal passwords. The real app will dim its window and let the system prompt appear on top.
2. **Packages are tied to their distro family.** A .deb can't be installed natively on Fedora, and an .rpm can't be installed on Ubuntu. The installer should detect this and explain it in plain language.
