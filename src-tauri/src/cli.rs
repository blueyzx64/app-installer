//! argv parsing and package path validation.
//!
//! A path reaches the backend only from argv, single-instance forwarding, a
//! file chooser or a drop event. Every one of them goes through
//! `validate_package_path` (ARCHITECTURE §12).

use std::path::{Path, PathBuf};

use crate::error::{CommandError, ErrorCode};

/// Largest package we accept: 8 GiB.
pub const MAX_PACKAGE_BYTES: u64 = 8 * 1024 * 1024 * 1024;

/// First non-flag argument after the program name, if any.
pub fn file_from_argv(argv: &[String]) -> Option<PathBuf> {
    argv.iter()
        .skip(1)
        .find(|arg| !arg.starts_with('-'))
        .map(PathBuf::from)
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
