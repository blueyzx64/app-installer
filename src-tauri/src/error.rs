//! `CommandError`: the only error type that crosses the IPC boundary.
//!
//! The frontend switches on `code`, never on message text (ARCHITECTURE §8).

use serde::{Deserialize, Serialize};
use specta::Type;

/// Stable error codes. Keep in sync with `docs/architecture/ipc-contract.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ErrorCode {
    PermissionDenied,
    Offline,
    MissingDependencies,
    Conflicts,
    NotEnoughSpace,
    CorruptFile,
    UntrustedSource,
    BackendUnavailable,
    NotFound,
    Busy,
    Unknown,
}

/// Serializable command failure: `{ code, detail }`.
///
/// `detail` is technical text for logs and the optional "Details" disclosure.
/// It is never shown as a sentence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[error("{code:?}: {detail:?}")]
pub struct CommandError {
    pub code: ErrorCode,
    pub detail: Option<String>,
}

impl CommandError {
    pub fn new(code: ErrorCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: Some(detail.into()),
        }
    }

    pub fn code(code: ErrorCode) -> Self {
        Self { code, detail: None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_camel_case_with_null_detail() {
        let json = serde_json::to_string(&CommandError::code(ErrorCode::NotFound)).unwrap();
        assert_eq!(json, r#"{"code":"notFound","detail":null}"#);
    }
}
