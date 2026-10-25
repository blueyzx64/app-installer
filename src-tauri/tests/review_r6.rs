//! R6 review proofs (logging case only).
use installer_lib::logging::redact;

#[test]
fn review_r6_redact_does_not_mangle_sibling_user_dir() {
    let out = redact("open /home/samuel/secret.txt", Some("/home/sam"));
    assert_eq!(out, "open /home/samuel/secret.txt");
}

#[test]
fn review_r6_redact_home_root_slash() {
    assert_eq!(redact("/usr/bin/x", Some("/")), "/usr/bin/x");
}
