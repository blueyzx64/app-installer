//! Installed apps and their identity.

use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::package::IconData;
use crate::version::Version;

const MAX_SLUG_LEN: usize = 64;
const FALLBACK_SLUG: &str = "app";

/// Stable app identity: a slug that is safe in file names and desktop IDs
/// (`[a-z0-9-]`, no leading/trailing/double hyphen, 1..=64 chars).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AppId(String);

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("not a valid app id: {0:?}")]
pub struct InvalidAppId(pub String);

impl AppId {
    /// Make a slug from a display or machine name. Always returns a valid id.
    ///
    /// ASCII letters/digits are kept (lower-cased); every other run of characters
    /// becomes one hyphen. Names with no ASCII letters or digits become `app`.
    pub fn from_name(name: &str) -> AppId {
        let mut slug = String::with_capacity(name.len());
        for ch in name.chars() {
            if ch.is_ascii_alphanumeric() {
                slug.push(ch.to_ascii_lowercase());
            } else if !slug.ends_with('-') {
                slug.push('-');
            }
        }
        slug.truncate(MAX_SLUG_LEN); // ASCII only, so any index is a char boundary
        let slug = slug.trim_matches('-');
        if slug.is_empty() {
            AppId(FALLBACK_SLUG.to_owned())
        } else {
            AppId(slug.to_owned())
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::str::FromStr for AppId {
    type Err = InvalidAppId;

    /// Accept only ids that `from_name` could have produced.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if AppId::from_name(s).0 == s {
            Ok(AppId(s.to_owned()))
        } else {
            Err(InvalidAppId(s.to_owned()))
        }
    }
}

impl fmt::Display for AppId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AppSource {
    AppImage,
    SystemPackage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledApp {
    pub id: AppId,
    /// Machine name; for system packages this is the package name used to remove it.
    pub name: String,
    pub display_name: String,
    pub version: Version,
    pub icon: Option<IconData>,
    pub source: AppSource,
    /// Seconds since the Unix epoch. The shell formats it as ISO-8601.
    pub installed_at: i64,
    /// Desktop file id (`foo.desktop`) if the app can be launched.
    pub desktop_id: Option<String>,
    /// Where the app file lives (AppImage only).
    pub path: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_table() {
        let long_in = "x".repeat(100);
        let long_out = "x".repeat(64);
        let rows = [
            ("Lumen Notes", "lumen-notes"),
            ("lumen-notes", "lumen-notes"),
            ("  Lumen   Notes  ", "lumen-notes"),
            ("Lumen_Notes 2.0", "lumen-notes-2-0"),
            ("C++ Editor", "c-editor"),
            ("../../etc/passwd", "etc-passwd"),
            ("a/b\\c", "a-b-c"),
            ("Caf\u{e9} Reader", "caf-reader"),
            ("\u{65e5}\u{672c}\u{8a9e}", "app"),
            ("", "app"),
            ("---", "app"),
            ("UPPER", "upper"),
            (long_in.as_str(), long_out.as_str()),
        ];
        for (input, want) in rows {
            assert_eq!(AppId::from_name(input).as_str(), want, "{input:?}");
        }
    }

    #[test]
    fn slug_truncation_does_not_leave_trailing_hyphen() {
        let name = format!("{}-{}", "a".repeat(63), "b".repeat(10));
        let id = AppId::from_name(&name);
        assert_eq!(id.as_str(), "a".repeat(63));
    }

    #[test]
    fn slug_is_idempotent_and_safe() {
        for input in ["Lumen Notes", "../x", "\u{1f600}", "A  B--C", "x.y.z"] {
            let once = AppId::from_name(input);
            let twice = AppId::from_name(once.as_str());
            assert_eq!(once, twice, "{input:?}");
            assert!(
                once.as_str()
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'),
                "{input:?}"
            );
            assert!(!once.as_str().starts_with('-') && !once.as_str().ends_with('-'));
        }
    }

    #[test]
    fn parse_accepts_only_slugs() {
        assert!("lumen-notes".parse::<AppId>().is_ok());
        assert!("Lumen".parse::<AppId>().is_err());
        assert!("a b".parse::<AppId>().is_err());
        assert!("../x".parse::<AppId>().is_err());
        assert!("".parse::<AppId>().is_err());
    }
}
