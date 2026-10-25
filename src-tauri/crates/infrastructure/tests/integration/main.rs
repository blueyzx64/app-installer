//! Integration harness for the D-Bus adapters (PackageKit, dnf5daemon, polkit).
//!
//! Every test here is `#[ignore]`: they need a real system bus with PackageKit, so they run
//! only inside a Fedora 45 container (see README.md). `cargo test --workspace` skips them.
//!
//! HUMAN: run these in the fedora:45 container and on a real GNOME session.

use std::env;

/// True when the harness runs in the container prepared by README.md.
fn in_fedora_container() -> bool {
    env::var("INSTALLER_IT_CONTAINER").as_deref() == Ok("1")
}

/// Fixture directory shared with the parser tests.
fn fixtures_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

#[test]
#[ignore = "needs the fedora:45 container with a system bus and PackageKit"]
fn harness_sees_container_and_fixtures() {
    assert!(
        in_fedora_container(),
        "set INSTALLER_IT_CONTAINER=1 inside the fedora:45 container"
    );
    assert!(
        env::var("DBUS_SYSTEM_BUS_ADDRESS").is_ok()
            || std::path::Path::new("/run/dbus/system_bus_socket").exists()
    );
    let _ = fixtures_dir();
}

#[test]
#[ignore = "needs PackageKit; implement with lane B (W2)"]
fn packagekit_resolves_installed_version() {
    todo!("W2: ask PackageKit for the installed version of a known package, expect Some(version)");
}

#[test]
#[ignore = "needs PackageKit and a local repo; implement with lane B (W2)"]
fn packagekit_installs_local_rpm_fixture() {
    todo!(
        "W2: install tests/fixtures/*.rpm via InstallFiles, expect progress phases then Installed"
    );
}

#[test]
#[ignore = "needs polkit agent stub; implement with lane B (W2)"]
fn dismissed_polkit_dialog_is_cancel_not_error() {
    todo!("W2: deny auth through a stub agent, expect InstallOutcome::Cancelled");
}

#[test]
#[ignore = "needs dnf5daemon; implement with lane B (W2)"]
fn dnf5daemon_fallback_installs_when_packagekit_missing() {
    todo!("W2: stop packagekitd, expect the dnf5daemon adapter to install the fixture");
}
