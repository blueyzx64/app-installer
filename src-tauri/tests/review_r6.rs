//! R6 adversarial review proofs. Each test asserts the CORRECT behaviour;
//! a failing test is a finding.
use installer_lib::cli::{file_from_argv, launch_path_from_argv, validate_package_path};
use installer_lib::dropzone::valid_drops;
use installer_lib::error::ErrorCode;
use installer_lib::logging::redact;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

fn args(l: &[&str]) -> Vec<String> {
    l.iter().map(|s| s.to_string()).collect()
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("review-r6-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn mkfifo(p: &Path) {
    let ok = std::process::Command::new("mkfifo")
        .arg(p)
        .status()
        .unwrap()
        .success();
    assert!(ok);
}

// ---- preview id parsing (domain, reached via install/cancel_install) ----
#[test]
fn review_r6_preview_id_bad_inputs_error_not_panic() {
    use installer_domain::PreviewId;
    for bad in [
        "",
        "p",
        "x5",
        "P5",
        "p-1",
        "p 5",
        "p5 ",
        "p1e3",
        "p99999999999999999999999",
        "p\u{0}",
        "é",
        "pé",
        "p٣",
    ] {
        assert!(bad.parse::<PreviewId>().is_err(), "accepted {bad:?}");
    }
}

#[test]
fn review_r6_preview_id_rejects_noncanonical_plus_and_leading_zero() {
    use installer_domain::PreviewId;
    for alias in ["p+5", "p05"] {
        assert!(
            alias.parse::<PreviewId>().is_err(),
            "alias accepted: {alias}"
        );
    }
}

// ---- argv ----
#[test]
fn review_r6_argv_double_dash_allows_dash_filename() {
    assert_eq!(
        file_from_argv(&args(&["installer", "--", "-weird.rpm"])),
        Some(PathBuf::from("-weird.rpm"))
    );
}

#[test]
fn review_r6_argv_empty_string_is_no_file() {
    assert_eq!(
        launch_path_from_argv(&args(&["installer", ""]), Path::new("/tmp")),
        None
    );
}

#[test]
fn review_r6_argv_file_uri_is_resolved() {
    let d = scratch("uri");
    let f = d.join("a b.AppImage");
    std::fs::write(&f, b"x").unwrap();
    let uri = format!("file://{}", f.to_string_lossy().replace(' ', "%20"));
    let got = launch_path_from_argv(&args(&["installer", &uri]), &d).unwrap();
    assert_eq!(got, f.canonicalize().unwrap());
}

// ---- validate_package_path ----
#[test]
fn review_r6_validate_rejects_special_files_without_panic() {
    let d = scratch("special");
    let fifo = d.join("p.rpm");
    mkfifo(&fifo);
    assert!(validate_package_path(fifo.to_str().unwrap()).is_err());
    let z = d.join("z.AppImage");
    symlink("/dev/zero", &z).unwrap();
    assert_eq!(
        validate_package_path(z.to_str().unwrap()).unwrap_err().code,
        ErrorCode::UntrustedSource
    );
    let e = d.join("e.rpm");
    symlink("/proc/self/environ", &e).unwrap();
    assert!(validate_package_path(e.to_str().unwrap()).is_err());
    let dl = d.join("d.rpm");
    symlink("/tmp", &dl).unwrap();
    assert!(validate_package_path(dl.to_str().unwrap()).is_err());
    let dang = d.join("dang.rpm");
    symlink("/nonexistent/x", &dang).unwrap();
    assert_eq!(
        validate_package_path(dang.to_str().unwrap())
            .unwrap_err()
            .code,
        ErrorCode::NotFound
    );
    assert!(validate_package_path("a\0b").is_err());
    assert!(validate_package_path("").is_err());
}

// ---- dropzone ----
#[test]
fn review_r6_dropzone_rejects_dirs_fifos_devs() {
    let d = scratch("drop");
    let fifo = d.join("f.rpm");
    mkfifo(&fifo);
    let out = valid_drops(&[
        d.clone(),
        fifo,
        PathBuf::from("/dev/zero"),
        PathBuf::from("/proc/self/environ"),
        PathBuf::from(""),
    ]);
    assert!(out.is_empty(), "{out:?}");
}

// ---- logging ----
#[test]
fn review_r6_redact_does_not_mangle_sibling_user_dir() {
    let out = redact("open /home/samuel/secret.txt", Some("/home/sam"));
    assert_eq!(out, "open /home/samuel/secret.txt");
}

#[test]
fn review_r6_redact_home_root_slash() {
    assert_eq!(redact("/usr/bin/x", Some("/")), "/usr/bin/x");
}

// ---- re-verification round: attacks on the fixes ----
#[test]
fn review_r6_rv_uri_nul_byte_is_error_not_panic() {
    let p = launch_path_from_argv(
        &args(&["installer", "file:///tmp/a%00b.rpm"]),
        Path::new("/"),
    )
    .unwrap();
    assert!(validate_package_path(&p.to_string_lossy()).is_err());
}

#[test]
fn review_r6_rv_uri_dotdot_and_slash_escape_canonicalised() {
    let d = scratch("uridot");
    let f = d.join("x.rpm");
    std::fs::write(&f, b"x").unwrap();
    let uri = format!("file://{}/sub%2F..%2Fx.rpm", d.display());
    std::fs::create_dir_all(d.join("sub")).unwrap();
    let p = launch_path_from_argv(&args(&["installer", &uri]), Path::new("/")).unwrap();
    assert_eq!(p, f.canonicalize().unwrap());
}

#[test]
fn review_r6_rv_double_dash_then_only_empty_is_none() {
    assert_eq!(file_from_argv(&args(&["installer", "--", ""])), None);
    assert_eq!(
        file_from_argv(&args(&["installer", "--", "--"])),
        Some(PathBuf::from("--"))
    );
}

#[test]
fn review_r6_rv_non_utf8_uri_file_survives_to_open_payload() {
    use std::os::unix::ffi::OsStrExt;
    let d = scratch("nonutf8");
    let name = std::ffi::OsStr::from_bytes(b"caf\xe9.rpm");
    let f = d.join(name);
    std::fs::write(&f, b"x").unwrap();
    let uri = format!("file://{}/caf%E9.rpm", d.display());
    let payload =
        installer_lib::cli::open_file_payload(&args(&["installer", &uri]), Path::new("/")).unwrap();
    // frontend gets payload.path and calls open_package(path) -> validate
    assert!(
        validate_package_path(&payload.path).is_ok(),
        "latin-1 named file lost: {:?}",
        payload.path
    );
}

#[test]
fn review_r6_rv_redact_needs_left_boundary_too() {
    // HOME=/home/sam; /var/home/sam/... (Atomic symlink target) or /mnt/home/sam
    let out = redact("open /var/home/sam/a.rpm", Some("/home/sam"));
    assert!(!out.contains("/var~"), "mangled: {out}");
}

#[test]
fn review_r6_rv_redact_hides_username_in_removable_media_path() {
    // F-60 "no personal data": open_package span logs the full path (open_package.rs:53)
    let out = redact(
        "open_package{path=/run/media/sam/USB/a.rpm}",
        Some("/home/sam"),
    );
    assert!(!out.contains("/sam/"), "username leaked: {out}");
}

#[test]
fn review_r6_rv_redact_boundary_variants() {
    let h = Some("/home/sam");
    assert_eq!(redact("/home/sam", h), "~");
    assert_eq!(redact("/home/sam/", h), "~/");
    assert_eq!(redact("x=/home/sam/a y=/home/sam", h), "x=~/a y=~");
    assert_eq!(redact("/home/sam.bak/a", h), "/home/sam.bak/a");
    assert_eq!(redact("/home/sam/home/sam/a", h), "~/home/sam/a");
}
