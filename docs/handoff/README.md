# Linux Package Installer — Project Handoff

> **Historical.** This is the original handoff, kept as delivered (only this note was added). Paths below refer to the original zip; the prototype now lives in `design/prototype/`. Current plans: `docs/architecture/ARCHITECTURE.md`.

A macOS-style "drag to Applications" installer for Linux that handles **.rpm**, **.deb** and **AppImage** files, aimed at non-technical users.

Prepared: 25 October 2026

## What's in this folder

| File | What it is |
|---|---|
| `01-conversation-summary.md` | How the idea developed, step by step |
| `02-decisions.md` | Every decision made so far, with the reasoning, plus open questions |
| `03-technical-plan.md` | How to build it with Tauri 2: architecture, install logic, packaging, CI, first milestones |
| `prototype/index.html` | The interactive prototype as a standalone page. Open it in any browser, no setup needed |
| `prototype/design-canvas-source/Main.dc.html` | The original source from the Claude design canvas (needs the canvas runtime; kept for reference) |

## Try the prototype

Open `prototype/index.html` in a browser, then:

1. Pick a package type at the top (AppImage, .deb or .rpm).
2. Double-click the package in the Downloads window.
3. The first time, choose **Make Default** or **Not Now**.
4. Drag the app icon onto **Applications**.
5. For .deb and .rpm, enter any password and click **Allow**.

The live version is also on the Claude design canvas: https://claude.ai/artifact/3xkdDRSJ12KZjRQVeFVEnJ (private until shared from its Share menu).

## The short version

- **Framework:** Tauri 2 (Rust back end, HTML/CSS/JS front end). The prototype's UI carries over directly.
- **Installing .rpm/.deb:** PackageKit over D-Bus, the same code on Fedora and Ubuntu, with polkit for the password.
- **Installing AppImages:** copy to `~/Applications` and add a menu entry, with no password needed.
- **Default installer:** a `.desktop` file declaring the package MIME types, plus `xdg-mime default`.
- **Shipping:** `.rpm` first (Fedora), plus `.deb` and AppImage from the same Tauri build.
