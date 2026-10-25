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
    /// ASCII letters/digits are kept (lower-cased); common Latin accents are
    /// stripped (`é` becomes `e`); `+` becomes `plus`; every other run of characters
    /// becomes one hyphen. If letters or digits of the name could not be kept
    /// (CJK, Cyrillic, ...), a short stable hash of the whole name is appended
    /// (`app-1a2b3c4d` when nothing is left), so different names keep different ids.
    pub fn from_name(name: &str) -> AppId {
        let mut slug = String::with_capacity(name.len());
        let mut lossy = false;
        let sep = |slug: &mut String| {
            if !slug.is_empty() && !slug.ends_with('-') {
                slug.push('-');
            }
        };
        for ch in name.chars() {
            if ch.is_ascii_alphanumeric() {
                slug.push(ch.to_ascii_lowercase());
            } else if ch == '+' {
                sep(&mut slug);
                slug.push_str("plus-");
            } else if let Some(folded) = fold_accent(ch) {
                slug.push_str(folded);
            } else {
                if ch.is_alphanumeric() {
                    lossy = true;
                }
                sep(&mut slug);
            }
        }
        let hash = format!("{:08x}", fnv1a32(name));
        let room = if lossy {
            MAX_SLUG_LEN - 1 - hash.len()
        } else {
            MAX_SLUG_LEN
        };
        slug.truncate(room); // ASCII only, so any index is a char boundary
        let base = slug.trim_matches('-');
        match (base.is_empty(), lossy) {
            (true, false) => AppId(FALLBACK_SLUG.to_owned()),
            (true, true) => AppId(format!("{FALLBACK_SLUG}-{hash}")),
            (false, false) => AppId(base.to_owned()),
            (false, true) => AppId(format!("{base}-{hash}")),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// FNV-1a, 32 bit. Stable across runs and platforms (unlike `DefaultHasher`).
fn fnv1a32(s: &str) -> u32 {
    s.bytes().fold(0x811c_9dc5_u32, |h, b| {
        (h ^ u32::from(b)).wrapping_mul(0x0100_0193)
    })
}

/// ASCII form of common Latin letters with accents. `None` for anything else.
fn fold_accent(ch: char) -> Option<&'static str> {
    let lower = ch.to_lowercase().next()?;
    const TABLE: [(&str, &str); 19] = [
        ("àáâãäåāăą", "a"),
        ("çćĉċč", "c"),
        ("ďđ", "d"),
        ("èéêëēĕėęě", "e"),
        ("ĝğ", "g"),
        ("ìíîïīĭįı", "i"),
        ("ł", "l"),
        ("ñńň", "n"),
        ("òóôõöøōŏő", "o"),
        ("ř", "r"),
        ("śşš", "s"),
        ("ţť", "t"),
        ("ùúûüūŭůűų", "u"),
        ("ýÿ", "y"),
        ("źżž", "z"),
        ("ß", "ss"),
        ("æ", "ae"),
        ("œ", "oe"),
        ("þ", "th"),
    ];
    TABLE
        .iter()
        .find(|(chars, _)| chars.contains(lower))
        .map(|(_, out)| *out)
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
            ("C++ Editor", "c-plus-plus-editor"),
            ("../../etc/passwd", "etc-passwd"),
            ("a/b\\c", "a-b-c"),
            ("Caf\u{e9} Reader", "cafe-reader"),
            ("\u{2122}\u{2014}", "app"),
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

#[cfg(test)]
mod prop {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn slug_is_always_a_valid_id(chars in proptest::collection::vec(any::<char>(), 0..80)) {
            let name: String = chars.into_iter().collect();
            let id = AppId::from_name(&name);
            let s = id.as_str();

            prop_assert!(!s.is_empty());
            prop_assert!(s.len() <= 64);
            prop_assert!(s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'));
            prop_assert!(!s.starts_with('-') && !s.ends_with('-'));
            prop_assert!(!s.contains("--"));
            prop_assert_eq!(AppId::from_name(s), id.clone());
            prop_assert!(s.parse::<AppId>().is_ok());
        }
    }

    #[test]
    fn plus_and_non_ascii_names_stay_distinct() {
        let id = |n: &str| AppId::from_name(n);
        assert_eq!(id("C++ Studio").as_str(), "c-plus-plus-studio");
        assert_ne!(id("C++ Studio"), id("C Studio"));
        assert_eq!(id("Caf\u{e9} Reader").as_str(), "cafe-reader");
        assert_eq!(id("\u{c5}ngstr\u{f6}m").as_str(), "angstrom");
        let (ja, zh) = (id("\u{65e5}\u{672c}\u{8a9e}"), id("\u{4e2d}\u{6587}"));
        assert_ne!(ja, zh);
        assert!(ja.as_str().starts_with("app-"), "{ja}");
        assert_eq!(ja, id("\u{65e5}\u{672c}\u{8a9e}"), "stable");
        let mixed = id("Lumen \u{7b14}\u{8bb0}");
        assert!(mixed.as_str().starts_with("lumen-"), "{mixed}");
        assert_ne!(mixed, id("Lumen \u{65e5}\u{8a18}"));
        // Hash ids and long lossy names are valid ids and round-trip.
        for n in [ja.as_str(), mixed.as_str()] {
            assert_eq!(n.parse::<AppId>().unwrap().as_str(), n);
        }
        let long = id(&format!("{}\u{65e5}", "x".repeat(100)));
        assert!(long.as_str().len() <= 64);
        assert_eq!(long.as_str().parse::<AppId>().unwrap(), long);
    }
}
