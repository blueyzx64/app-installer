#!/usr/bin/env bash
# D-Bus integration tests: Fedora 45 container with PackageKit, then ignored Rust tests.
# HUMAN: needs podman and network. Not run in CI by default.
set -euo pipefail
cd "$(dirname "$0")/.."
image="${INTEGRATION_IMAGE:-registry.fedoraproject.org/fedora:45}"
podman run --rm -v "$PWD:/src:Z" -w /src/src-tauri "$image" bash -euxc '
  dnf install -y --setopt=install_weak_deps=False \
    gcc gcc-c++ pkgconf-pkg-config dbus-daemon PackageKit \
    webkit2gtk4.1-devel gtk3-devel dbus-devel openssl-devel librsvg2-devel libsoup3-devel curl
  curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
  . "$HOME/.cargo/env"
  mkdir -p /run/dbus
  dbus-daemon --system --fork
  /usr/libexec/packagekitd &
  sleep 2
  cargo test --workspace -- --ignored
'
