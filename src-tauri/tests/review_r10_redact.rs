//! R10 reverify: the home path itself, followed by a quote, space or colon, must be redacted too.
use installer_lib::logging::redact;

#[test]
fn review_r10_home_followed_by_quote_is_redacted() {
    let out = redact(r#"cwd="/home/sam" ok"#, Some("/home/sam"));
    assert!(!out.contains("/home/sam"), "home leaked: {out}");
}

#[test]
fn review_r10_home_followed_by_colon_or_space_is_redacted() {
    for text in ["dir /home/sam: gone", "in /home/sam (cwd)"] {
        let out = redact(text, Some("/home/sam"));
        assert!(!out.contains("/home/sam"), "home leaked: {out}");
    }
}

#[test]
fn review_r10_longer_user_name_is_kept() {
    // The fix's intent: /home/samuel is someone else's path.
    assert_eq!(
        redact("/home/samuel/x", Some("/home/sam")),
        "/home/samuel/x"
    );
    assert_eq!(redact("/home/sam/x", Some("/home/sam")), "~/x");
}
