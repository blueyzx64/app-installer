# ADR-0004: PackageKit primary, dnf5daemon fallback

- **Status:** Proposed (confirm in M0 spike 2)
- **Date:** 2026-10-25

## Context
PackageKit offers one D-Bus API for dnf (Fedora) and apt (Ubuntu), and it triggers polkit itself. Fedora 44 switched PackageKit to a libdnf5 backend. In parallel, Fedora's GNOME Software has been moving to a downstream dnf5daemon plugin, so PackageKit's long-term presence on Workstation is less certain.

## Decision
Implement `SystemPackages` with a PackageKit adapter as the primary path. Add a `dnf5daemon` adapter (`org.rpm.dnf.v0`) behind a cargo feature. At startup the composition root probes for an activatable `org.freedesktop.PackageKit` and falls back to dnf5daemon on RPM-based hosts. Our RPM declares `Requires: PackageKit` until the spike shows otherwise.

## Consequences
- Application code is unaffected by which backend is used.
- There are two adapters to test on Fedora; the dnf5daemon one is Fedora-only.

## To verify
- Is PackageKit installed and activatable on a fresh Fedora 45 Workstation?
- The dnf5daemon local-file install API (`Rpm`/`Goal` interfaces), and which polkit action it uses.
