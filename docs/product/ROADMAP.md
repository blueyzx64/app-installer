# Roadmap

Each milestone ends with something you can demo. Feature IDs refer to [FEATURES.md](FEATURES.md).

## M0: Skeleton and spikes (de-risk first)

- Scaffold: `pnpm create tauri-app` (SvelteKit, TS), then reshape it to [folder-structure.md](../architecture/folder-structure.md). Cargo workspace with `domain` / `application` / `infrastructure` crates. CI running lint and test.
- **Spike 1 (window):** `decorations:false` + `transparent:true` on GNOME 51 Wayland. Check rounded corners, drag region, shadow and the look when unfocused. Record the result in ADR-0006.
- **Spike 2 (backend):** on a **fresh Fedora 45 Workstation** VM, is `org.freedesktop.PackageKit` activatable by default? Does `InstallFiles` on a local unsigned RPM trigger the GNOME polkit dialog? Record the result in ADR-0004.
- **Spike 3 (types):** `tauri-specta` generates bindings for a Channel command. Record the result in ADR-0009.
- Port the prototype's visuals into `src/lib/ui` + `features/installer`, running on mock gateways (F-61).

**Demo:** the prototype flow running inside a real Tauri window with mock data.

## M1: AppImage end-to-end (no root)

F-01, F-02, F-03, F-04, F-20, F-21, F-22, F-25 (user scope), F-29, F-53, F-54.

**Demo:** double-click a real AppImage in Files, drag it, and it appears in the GNOME app grid and launches.

## M2: RPM via PackageKit

F-05, F-10, F-11, F-12, F-23, F-24, F-25, F-26, (F-31 if spike 2 says so).

**Demo:** install a third-party RPM (e.g. a small signed vendor RPM) through the GNOME polkit dialog; cancel the dialog and see the icon spring back.

## M3: Default handler and packaging

F-50, F-51, F-60, F-62, plus a custom `.desktop` template and metainfo (F-52).

**Demo:** install the built RPM of the installer itself on a clean VM; double-clicking any package now opens it.

## M4: Library and uninstall (v1 scope)

F-40, F-41, F-42, F-43, F-06, F-07.

## M5: Edge cases and polish → 1.0

F-13, F-14, F-15, F-27, F-28, F-55, an accessibility pass, visual regression baselines, Copr (F-63) and signing (F-64).

## After 1.0

Ubuntu target (F-09, F-30, `.deb` bundle, PPA), translations (F-56), .rpm icons (F-08), and parked ideas.
