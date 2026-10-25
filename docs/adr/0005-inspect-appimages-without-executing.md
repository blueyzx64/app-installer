# ADR-0005: Inspect AppImages without executing them

- **Status:** Accepted
- **Date:** 2026-10-25

## Context
`--appimage-extract` runs the AppImage's own runtime, which is untrusted code from the internet, before the user has agreed to install anything.

## Decision
Locate the SquashFS offset from the ELF header and read it with the `backhand` crate. Read only the root `*.desktop`, `.DirIcon` (resolving symlinks *inside* the image) and AppStream metainfo. Generate our own `.desktop` file; never copy `Exec=` from the image.

## Consequences
- Opening a file never runs it (enforced by a test that forbids `std::process` in the `inspect` module).
- DwarFS and type-1 (ISO) AppImages are unsupported in v1.
