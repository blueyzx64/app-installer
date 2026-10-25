# ADR-0007: polkit owns authentication

- **Status:** Accepted
- **Date:** 2026-10-25

## Context
The prototype showed a custom password dialog. A real app must never collect admin passwords: it would train users to type passwords into arbitrary windows.

## Decision
Use no password UI. When PackageKit reports `WAITING_FOR_AUTH`, dim the installer and show "Waiting for permission…" while GNOME Shell's polkit dialog appears. A dismissed dialog counts as a cancel, not an error.

## Consequences
The auth dialog looks GNOME-native, not macOS-like. That is expected and correct.
