//! R3 review proof: an empty first os-release must not hide a good second one.
use installer_application::ports::HostProbe;
use installer_domain::DistroFamily;
use installer_infrastructure::host::OsReleaseProbe;

#[test]
fn empty_os_release_falls_back() {
    let d = std::env::temp_dir().join(format!("r3-osr-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("a"), "").unwrap();
    std::fs::write(d.join("b"), "ID=fedora\nVERSION_ID=45\n").unwrap();
    let h = OsReleaseProbe::with_roots(vec![d.join("a"), d.join("b")], vec![]).host();
    assert_eq!(h.family, DistroFamily::RpmBased);
    assert_eq!(h.version_id, "45");
    let _ = std::fs::remove_dir_all(d);
}
