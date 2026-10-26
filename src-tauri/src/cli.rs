//! argv parsing and package path validation.
//!
//! A path reaches the backend only from argv, single-instance forwarding, a
//! file chooser or a drop event. Every one of them goes through
//! `validate_package_path` (ARCHITECTURE §12).
//!
//! Wire form of a path (IPC carries strings): a UTF-8 path is sent as is. A
//! path that is not valid UTF-8 is sent as a `file://` URI with every byte
//! outside the safe set percent-encoded, and `validate_package_path` decodes
//! it again. So argv, drop and chooser paths stay lossless end to end, with no
//! server-side state.

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use crate::error::{CommandError, ErrorCode};
use crate::events::OpenFilePayload;

/// Largest package we accept: 8 GiB.
pub const MAX_PACKAGE_BYTES: u64 = 8 * 1024 * 1024 * 1024;

/// First path argument after the program name, if any.
/// Flags (`-x`) and empty arguments are skipped. Everything after `--` is a
/// path, so a file name that starts with a dash can still be opened.
/// Takes `OsString`s (from `std::env::args_os`) or `String`s.
pub fn file_from_argv<S: AsRef<OsStr>>(argv: &[S]) -> Option<PathBuf> {
    let mut only_paths = false;
    for arg in argv.iter().skip(1) {
        let arg = arg.as_ref();
        if arg.is_empty() {
            continue;
        }
        if !only_paths && arg == "--" {
            only_paths = true;
            continue;
        }
        if only_paths || !arg.as_bytes().starts_with(b"-") {
            return Some(PathBuf::from(arg));
        }
    }
    None
}

/// Path to its wire string (see the module doc). Lossless.
pub fn path_to_wire(path: &Path) -> String {
    match path.to_str() {
        Some(s) if !s.starts_with("file://") => s.to_owned(),
        _ => {
            let mut out = String::from("file://");
            for &b in path.as_os_str().as_bytes() {
                if b.is_ascii_alphanumeric() || b"/._-~".contains(&b) {
                    out.push(b as char);
                } else {
                    out.push_str(&format!("%{b:02X}"));
                }
            }
            out
        }
    }
}

/// Wire string back to a path. Plain paths pass through.
pub fn wire_to_path(raw: &str) -> PathBuf {
    decode_file_uri(PathBuf::from(raw))
}

/// `file:///a%20b` to `/a b`. Anything that is not a local `file://` URI is
/// returned unchanged.
fn decode_file_uri(raw: PathBuf) -> PathBuf {
    use std::os::unix::ffi::OsStringExt;

    let bytes = raw.as_os_str().as_bytes();
    let Some(rest) = bytes.strip_prefix(b"file://") else {
        return raw;
    };
    let rest = rest.strip_prefix(b"localhost").unwrap_or(rest);
    if !rest.starts_with(b"/") {
        return raw; // a remote host, or not a path: leave it alone
    }
    let mut out = Vec::with_capacity(rest.len());
    let mut i = 0;
    while i < rest.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        match (
            rest[i],
            rest.get(i + 1).and_then(|b| hex(*b)),
            rest.get(i + 2).and_then(|b| hex(*b)),
        ) {
            (b'%', Some(hi), Some(lo)) => {
                out.push((hi * 16 + lo) as u8);
                i += 3;
            }
            (b, _, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    PathBuf::from(std::ffi::OsString::from_vec(out))
}

/// Turn argv into an absolute path. Relative paths resolve against `cwd`
/// (the cwd of the process that was started, which differs for a forwarded
/// second launch). A path that cannot be canonicalised stays absolute but
/// unresolved, so `open_package` reports `notFound` instead of the file
/// silently vanishing.
pub fn launch_path_from_argv<S: AsRef<OsStr>>(argv: &[S], cwd: &Path) -> Option<PathBuf> {
    let raw = decode_file_uri(file_from_argv(argv)?);
    let absolute = if raw.is_absolute() {
        raw
    } else {
        cwd.join(raw)
    };
    Some(absolute.canonicalize().unwrap_or(absolute))
}

/// What the single-instance callback should do with a second launch's argv.
/// `None` means "just focus the window".
pub fn open_file_payload<S: AsRef<OsStr>>(argv: &[S], cwd: &Path) -> Option<OpenFilePayload> {
    launch_path_from_argv(argv, cwd).map(|p| OpenFilePayload {
        path: path_to_wire(&p),
    })
}

/// [`validate_path`] for a wire string from the frontend.
pub fn validate_package_path(raw: &str) -> Result<PathBuf, CommandError> {
    validate_path(&wire_to_path(raw))
}

/// Canonicalise, require a regular file of at most 8 GiB, and refuse `/proc` and `/dev`.
pub fn validate_path(raw: &Path) -> Result<PathBuf, CommandError> {
    let path = raw
        .canonicalize()
        .map_err(|e| CommandError::new(ErrorCode::NotFound, e.to_string()))?;
    if path.starts_with("/proc") || path.starts_with("/dev") {
        return Err(CommandError::new(
            ErrorCode::UntrustedSource,
            "path is in a special directory",
        ));
    }
    let meta = std::fs::metadata(&path)
        .map_err(|e| CommandError::new(ErrorCode::NotFound, e.to_string()))?;
    if !meta.is_file() {
        return Err(CommandError::new(
            ErrorCode::CorruptFile,
            "not a regular file",
        ));
    }
    if meta.len() > MAX_PACKAGE_BYTES {
        return Err(CommandError::new(ErrorCode::CorruptFile, "file too large"));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn argv_picks_first_non_flag() {
        assert_eq!(
            file_from_argv(&args(&["installer", "--foo", "/x/a.rpm"])),
            Some(PathBuf::from("/x/a.rpm"))
        );
        assert_eq!(file_from_argv(&args(&["installer"])), None);
    }

    #[test]
    fn launch_path_is_canonical_and_absolute() {
        let dir = std::env::temp_dir().join("installer-cli-launch");
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("a.AppImage"), b"x").unwrap();
        let got = launch_path_from_argv(&args(&["installer", "-x", "sub/../a.AppImage"]), &dir);
        assert_eq!(got, Some(dir.canonicalize().unwrap().join("a.AppImage")));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn missing_launch_path_stays_absolute() {
        let got = launch_path_from_argv(&args(&["installer", "nope.rpm"]), Path::new("/zzz"));
        assert_eq!(got, Some(PathBuf::from("/zzz/nope.rpm")));
    }

    #[test]
    fn payload_maps_argv_to_event_and_none_without_file() {
        let cwd = Path::new("/");
        assert_eq!(open_file_payload(&args(&["installer"]), cwd), None);
        assert_eq!(open_file_payload(&args(&["installer", "--new"]), cwd), None);
        let p = open_file_payload(&args(&["installer", "/nonexistent/b.rpm"]), cwd).unwrap();
        assert_eq!(p.path, "/nonexistent/b.rpm");
    }

    #[test]
    fn double_dash_empty_and_uri() {
        assert_eq!(
            file_from_argv(&args(&["installer", "", "--", "-a.rpm"])),
            Some(PathBuf::from("-a.rpm"))
        );
        assert_eq!(
            decode_file_uri(PathBuf::from("file:///a%20b/%E2%9C%93.rpm")),
            PathBuf::from("/a b/\u{2713}.rpm")
        );
        assert_eq!(
            decode_file_uri(PathBuf::from("file://localhost/x%zz")),
            PathBuf::from("/x%zz")
        );
        assert_eq!(
            decode_file_uri(PathBuf::from("file://host/x")),
            PathBuf::from("file://host/x")
        );
    }

    #[test]
    fn wire_round_trips_non_utf8_and_plain() {
        for raw in [
            &b"/tmp/caf\xe9 x%.rpm"[..],
            b"/tmp/a b.rpm",
            b"/tmp/file://odd",
        ] {
            let p = PathBuf::from(OsStr::from_bytes(raw));
            assert_eq!(wire_to_path(&path_to_wire(&p)), p, "{raw:?}");
        }
        assert_eq!(path_to_wire(Path::new("/tmp/a b.rpm")), "/tmp/a b.rpm");
    }

    #[test]
    fn argv_os_with_invalid_utf8_does_not_panic() {
        let argv = [
            std::ffi::OsString::from("installer"),
            OsStr::from_bytes(b"\xff.rpm").to_owned(),
        ];
        let p = launch_path_from_argv(&argv, Path::new("/zzz")).unwrap();
        assert_eq!(p.as_os_str().as_bytes(), b"/zzz/\xff.rpm");
    }

    #[test]
    fn rejects_missing_dir_and_dev() {
        assert_eq!(
            validate_package_path("/nonexistent/zzz").unwrap_err().code,
            ErrorCode::NotFound
        );
        assert_eq!(
            validate_package_path("/tmp").unwrap_err().code,
            ErrorCode::CorruptFile
        );
        assert_eq!(
            validate_package_path("/dev/null").unwrap_err().code,
            ErrorCode::UntrustedSource
        );
    }

    #[test]
    fn accepts_regular_file() {
        let file = std::env::temp_dir().join("installer-cli-test.bin");
        std::fs::write(&file, b"x").unwrap();
        let ok = validate_package_path(file.to_str().unwrap()).unwrap();
        assert!(ok.is_file());
        std::fs::remove_file(file).unwrap();
    }
}
