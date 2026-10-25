//! What the UI shows before the install.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::compat::Compatibility;
use crate::package::PackageMetadata;

/// Opaque handle for a preview held server-side. Wire form is a plain string like `p7`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PreviewId(u64);

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("not a valid preview id: {0:?}")]
pub struct InvalidPreviewId(pub String);

impl PreviewId {
    pub const fn new(n: u64) -> Self {
        Self(n)
    }
}

impl fmt::Display for PreviewId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "p{}", self.0)
    }
}

impl FromStr for PreviewId {
    type Err = InvalidPreviewId;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Canonical form only: digits, no sign, no leading zero (`p5`, not `p+5` or `p05`).
        s.strip_prefix('p')
            .filter(|n| {
                !n.is_empty()
                    && n.bytes().all(|b| b.is_ascii_digit())
                    && (n.len() == 1 || !n.starts_with('0'))
            })
            .and_then(|n| n.parse().ok())
            .map(PreviewId)
            .ok_or_else(|| InvalidPreviewId(s.to_owned()))
    }
}

impl Serialize for PreviewId {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for PreviewId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackagePreview {
    pub id: PreviewId,
    pub size_bytes: u64,
    pub meta: PackageMetadata,
    pub compat: Compatibility,
    /// Installing will ask for permission (system scope).
    pub needs_permission: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_round_trips() {
        let id = PreviewId::new(42);
        assert_eq!(id.to_string(), "p42");
        assert_eq!("p42".parse::<PreviewId>(), Ok(id));
        assert_eq!(serde_json::to_string(&id).unwrap(), "\"p42\"");
        assert_eq!(serde_json::from_str::<PreviewId>("\"p42\"").unwrap(), id);
    }

    #[test]
    fn bad_ids_rejected() {
        for bad in ["", "42", "p", "px", "p-1", "P1", "p1 "] {
            assert!(bad.parse::<PreviewId>().is_err(), "{bad:?}");
        }
    }

    #[test]
    fn parse_rejects_non_canonical_forms() {
        for bad in ["p+5", "p05", "p00", "p-1", "p", "5", "P5", "p 5", "p5 "] {
            assert!(bad.parse::<PreviewId>().is_err(), "{bad:?}");
        }
        for (good, n) in [("p0", 0), ("p5", 5), ("p120", 120)] {
            assert_eq!(good.parse::<PreviewId>().unwrap(), PreviewId::new(n));
        }
    }
}
