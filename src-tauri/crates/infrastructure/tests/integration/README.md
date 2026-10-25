# Infrastructure integration tests (HUMAN)

These tests talk to a real system D-Bus with PackageKit, dnf5daemon and polkit. They are all
`#[ignore]`d, so `cargo test --workspace` never runs them. Run them in a Fedora 45 container.

HUMAN: nobody has run this on Fedora yet. The container here is Ubuntu with no PackageKit.

## Run (podman, from the repo root)

```sh
podman run --rm -it --privileged \
  -v "$PWD":/src:Z -w /src/src-tauri \
  -e INSTALLER_IT_CONTAINER=1 \
  registry.fedoraproject.org/fedora:45 bash -c '
    dnf -y install rust cargo gcc pkgconf-pkg-config dbus-daemon PackageKit polkit dnf5daemon-server &&
    mkdir -p /run/dbus && dbus-daemon --system --fork &&
    (/usr/libexec/packagekitd &) && sleep 1 &&
    cargo test -p installer-infrastructure --test integration -- --ignored --test-threads=1
  '
```

Notes:

- `--privileged` is for systemd-less D-Bus and package installs. Use a throwaway container only.
- `--test-threads=1`: PackageKit has one transaction lock.
- Fixture packages live in `../fixtures/` (lane C owns them).
- Polkit has no agent in the container. Tests that need auth install a stub agent first (W2).
- The `todo!()` tests are placeholders for lane B. Replace them as the adapters land.
