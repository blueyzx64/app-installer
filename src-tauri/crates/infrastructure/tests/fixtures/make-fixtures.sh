#!/usr/bin/env bash
# Regenerates the tiny AppImage fixtures (ELF stub + SquashFS built by backhand's writer).
# No mksquashfs needed. Output: tests/fixtures/*.AppImage (each < 100 KB).
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo test -p installer-infrastructure --lib regenerate_fixtures -- --ignored
ls -l tests/fixtures/*.AppImage
