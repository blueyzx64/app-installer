//! argv parsing and package path validation.
//!
//! A path reaches the backend only from argv, single-instance forwarding, a
//! file chooser or a drop event. Every one of them goes through
//! `validate_package_path` (ARCHITECTURE §12).

use std::path::{Path, PathBuf};

use crate::error::{CommandError, ErrorCode};
use crate::events::OpenFilePayload;

/// Largest package we accept: 8 GiB.
pub const MAX_PACKAGE_BYTES: u64 = 8 * 1024 * 1024 * 1024;

/// First non-flag argument after the program name, if any.
pub fn file_from_argv(argv: &[String]) -> Option<PathBuf> {
    argv.iter()
        .skip(1)
        .find(|arg| !arg.starts_with('-'))
        .map(PathBuf::from)
}

/// Turn argv into an absolute path. Relative paths resolve against `cwd`
/// (the cwd of the process that was started, which differs for a forwarded
/// second launch). A path that cannot be canonicalised stays absolute but
/// unresolved, so `open_package` reports `notFound` instead of the file
/// silently vanishing.
pub fn launch_path_from_argv(argv: &[String], cwd: &Path) -> Option<PathBuf> {
    let raw = file_from_argv(argv)?;
    let absolute = if raw.is_absolute() {
        raw
    } else {
        cwd.join(raw)
    };
    Some(absolute.canonicalize().unwrap_or(absolute))
}

/// What the single-instance callback should do with a second launch's argv.
/// `None` means "just focus the window".
pub fn open_file_payload(argv: &[String], cwd: &Path) -> Option<OpenFilePayload> {
    launch_path_from_argv(argv, cwd).map(|p| OpenFilePayload {
        path: p.to_string_lossy().into_owned(),
    })
}

/// Canonicalise, require a regular file of at most 8 GiB, and refuse `/proc` and `/dev`.
pub fn validate_package_path(raw: &str) -> Result<PathBuf, CommandError> {
    let path = Path::new(raw)
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
