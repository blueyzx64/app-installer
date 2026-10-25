//! `CommandError`: the only error type that crosses the IPC boundary.
//!
//! The frontend switches on `code`, never on message text (ARCHITECTURE §8).
//! Cancelling is never an error; it is an outcome (`cancelled`).

use installer_domain as domain;
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

impl From<domain::ErrorCode> for ErrorCode {
    fn from(c: domain::ErrorCode) -> Self {
        use domain::ErrorCode as D;
        match c {
            D::PermissionDenied => Self::PermissionDenied,
            D::Offline => Self::Offline,
            D::MissingDependencies => Self::MissingDependencies,
            D::Conflicts => Self::Conflicts,
            D::NotEnoughSpace => Self::NotEnoughSpace,
            D::CorruptFile => Self::CorruptFile,
            D::UntrustedSource => Self::UntrustedSource,
            D::BackendUnavailable => Self::BackendUnavailable,
            D::NotFound => Self::NotFound,
            D::Busy => Self::Busy,
            D::Unknown => Self::Unknown,
        }
    }
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

/// D-006: a file with no known magic is `corruptFile` (the domain mapping does this).
impl From<domain::InspectError> for CommandError {
    fn from(e: domain::InspectError) -> Self {
        Self::new(e.code().into(), e.to_string())
    }
}

impl From<domain::BackendError> for CommandError {
    fn from(e: domain::BackendError) -> Self {
        use domain::BackendError as B;
        let code = match &e {
            B::Unavailable(_) => ErrorCode::BackendUnavailable,
            B::NotAuthorized => ErrorCode::PermissionDenied,
            B::Offline => ErrorCode::Offline,
            B::Failed(_) | B::Io(_) => ErrorCode::Unknown,
        };
        Self::new(code, e.to_string())
    }
}

/// `Cancelled` is an outcome, not an error. It maps to `unknown` only so the
/// conversion is total; no use case returns it through a `Result` to a command.
impl From<domain::InstallError> for CommandError {
    fn from(e: domain::InstallError) -> Self {
        let code = e.code().map_or(ErrorCode::Unknown, ErrorCode::from);
        Self {
            code,
            detail: e.detail().map(str::to_owned),
        }
    }
}

impl From<domain::InvalidPreviewId> for CommandError {
    fn from(e: domain::InvalidPreviewId) -> Self {
        Self::new(ErrorCode::NotFound, e.to_string())
    }
}

impl From<domain::InvalidAppId> for CommandError {
    fn from(e: domain::InvalidAppId) -> Self {
        Self::new(ErrorCode::NotFound, e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{BackendError, InspectError, InstallError, UnsupportedReason};

    #[test]
    fn serializes_camel_case_with_null_detail() {
        let json = serde_json::to_string(&CommandError::code(ErrorCode::NotFound)).unwrap();
        assert_eq!(json, r#"{"code":"notFound","detail":null}"#);
    }

    #[test]
    fn inspect_errors_map_per_d006() {
        let code = |e: InspectError| CommandError::from(e).code;
        assert_eq!(code(InspectError::NotFound), ErrorCode::NotFound);
        assert_eq!(code(InspectError::Corrupt), ErrorCode::CorruptFile);
        assert_eq!(
            code(InspectError::Unsupported(UnsupportedReason::UnknownFormat)),
            ErrorCode::CorruptFile
        );
        assert_eq!(code(InspectError::Io("x".into())), ErrorCode::Unknown);
    }

    #[test]
    fn backend_errors_map() {
        let code = |e: BackendError| CommandError::from(e).code;
        assert_eq!(
            code(BackendError::Unavailable("x".into())),
            ErrorCode::BackendUnavailable
        );
        assert_eq!(
            code(BackendError::NotAuthorized),
            ErrorCode::PermissionDenied
        );
        assert_eq!(code(BackendError::Offline), ErrorCode::Offline);
        assert_eq!(code(BackendError::Failed("x".into())), ErrorCode::Unknown);
        assert_eq!(code(BackendError::Io("x".into())), ErrorCode::Unknown);
    }

    #[test]
    fn install_errors_map() {
        let e = CommandError::from(InstallError::Unknown("boom".into()));
        assert_eq!(e.code, ErrorCode::Unknown);
        assert_eq!(e.detail.as_deref(), Some("boom"));
        assert_eq!(
            CommandError::from(InstallError::Conflicts).code,
            ErrorCode::Conflicts
        );
        assert_eq!(CommandError::from(InstallError::Busy).code, ErrorCode::Busy);
    }
}
