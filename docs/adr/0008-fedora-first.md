# ADR-0008: Fedora first, Ubuntu later

- **Status:** Accepted
- **Date:** 2026-10-25

## Decision
v1 targets Fedora Workstation 45 (GNOME 51), with Fedora 44 (GNOME 50) as the minimum. It supports `.rpm` and AppImage. `.deb` support stays in the architecture (`PackageFormat::Deb`, PackageKit apt backend, `DistroFamily::DebBased`) but is not tested or shipped until after 1.0.

## Consequences
- CI builds in `fedora:44` and smoke-tests in `fedora:45`.
- A `.deb` opened on Fedora shows the wrong-family message (F-10) from day one.
