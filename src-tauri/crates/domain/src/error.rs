//! Errors per layer, plus the stable classification the UI switches on.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::compat::UnsupportedReason;

/// Stable error code shared with the frontend (`ErrorCode` in ipc-contract.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

/// What a person can do about an error. Drives which buttons the UI offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UserAction {
    /// Trying again may work (after fixing something, or just later).
    TryAgain,
    /// Nothing helps with this file; close it or pick another.
    Close,
    /// Point at help; we cannot say more.
    GetHelp,
}

impl ErrorCode {
    pub fn user_action(self) -> UserAction {
        match self {
            ErrorCode::PermissionDenied
            | ErrorCode::Offline
            | ErrorCode::NotEnoughSpace
            | ErrorCode::Busy => UserAction::TryAgain,
            ErrorCode::MissingDependencies
            | ErrorCode::Conflicts
            | ErrorCode::CorruptFile
            | ErrorCode::UntrustedSource
            | ErrorCode::NotFound => UserAction::Close,
            ErrorCode::BackendUnavailable | ErrorCode::Unknown => UserAction::GetHelp,
        }
    }
}

/// Reading a package failed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InspectError {
    #[error("file not found or not readable")]
    NotFound,
    #[error("file is damaged or not a package")]
    Corrupt,
    #[error("package kind is not supported: {0:?}")]
    Unsupported(UnsupportedReason),
    #[error("io error: {0}")]
    Io(String),
}

impl InspectError {
    pub fn code(&self) -> ErrorCode {
        match self {
            InspectError::NotFound => ErrorCode::NotFound,
            InspectError::Corrupt | InspectError::Unsupported(_) => ErrorCode::CorruptFile,
            InspectError::Io(_) => ErrorCode::Unknown,
        }
    }
}

/// A system service (PackageKit, dnf5daemon, ledger file, ...) failed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BackendError {
    #[error("backend unavailable: {0}")]
    Unavailable(String),
    #[error("not authorized")]
    NotAuthorized,
    #[error("offline")]
    Offline,
    #[error("backend failed: {0}")]
    Failed(String),
    #[error("io error: {0}")]
    Io(String),
}

/// Installing or removing failed (or was cancelled).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InstallError {
    /// The person dismissed the permission dialog or cancelled. Not a failure.
    #[error("cancelled")]
    Cancelled,
    #[error("permission denied")]
    PermissionDenied,
    #[error("offline")]
    Offline,
    #[error("missing dependencies")]
    MissingDependencies,
    #[error("conflicts with installed packages")]
    Conflicts,
    #[error("not enough space")]
    NotEnoughSpace,
    #[error("corrupt file")]
    CorruptFile,
    #[error("untrusted source")]
    UntrustedSource,
    #[error("backend unavailable")]
    BackendUnavailable,
    #[error("not found")]
    NotFound,
    #[error("another operation is running")]
    Busy,
    #[error("unknown error: {0}")]
    Unknown(String),
}

impl InstallError {
    /// `None` for `Cancelled`, which is an outcome and not an error to show.
    pub fn code(&self) -> Option<ErrorCode> {
        Some(match self {
            InstallError::Cancelled => return None,
            InstallError::PermissionDenied => ErrorCode::PermissionDenied,
            InstallError::Offline => ErrorCode::Offline,
            InstallError::MissingDependencies => ErrorCode::MissingDependencies,
            InstallError::Conflicts => ErrorCode::Conflicts,
            InstallError::NotEnoughSpace => ErrorCode::NotEnoughSpace,
            InstallError::CorruptFile => ErrorCode::CorruptFile,
            InstallError::UntrustedSource => ErrorCode::UntrustedSource,
            InstallError::BackendUnavailable => ErrorCode::BackendUnavailable,
            InstallError::NotFound => ErrorCode::NotFound,
            InstallError::Busy => ErrorCode::Busy,
            InstallError::Unknown(_) => ErrorCode::Unknown,
        })
    }

    pub fn user_action(&self) -> Option<UserAction> {
        self.code().map(ErrorCode::user_action)
    }

    /// Technical detail for the "Show details" disclosure. Never shown by default.
    pub fn detail(&self) -> Option<&str> {
        match self {
            InstallError::Unknown(d) => Some(d),
            _ => None,
        }
    }
}

impl From<BackendError> for InstallError {
    fn from(e: BackendError) -> Self {
        match e {
            BackendError::Unavailable(_) => InstallError::BackendUnavailable,
            BackendError::NotAuthorized => InstallError::PermissionDenied,
            BackendError::Offline => InstallError::Offline,
            BackendError::Failed(d) | BackendError::Io(d) => InstallError::Unknown(d),
        }
    }
}

impl From<InspectError> for InstallError {
    fn from(e: InspectError) -> Self {
        match e {
            InspectError::NotFound => InstallError::NotFound,
            InspectError::Corrupt | InspectError::Unsupported(_) => InstallError::CorruptFile,
            InspectError::Io(d) => InstallError::Unknown(d),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_error_code_table() {
        let rows = [
            (
                InstallError::PermissionDenied,
                Some(ErrorCode::PermissionDenied),
            ),
            (InstallError::Offline, Some(ErrorCode::Offline)),
            (
                InstallError::MissingDependencies,
                Some(ErrorCode::MissingDependencies),
            ),
            (InstallError::Conflicts, Some(ErrorCode::Conflicts)),
            (
                InstallError::NotEnoughSpace,
                Some(ErrorCode::NotEnoughSpace),
            ),
            (InstallError::CorruptFile, Some(ErrorCode::CorruptFile)),
            (
                InstallError::UntrustedSource,
                Some(ErrorCode::UntrustedSource),
            ),
            (
                InstallError::BackendUnavailable,
                Some(ErrorCode::BackendUnavailable),
            ),
            (InstallError::NotFound, Some(ErrorCode::NotFound)),
            (InstallError::Busy, Some(ErrorCode::Busy)),
            (InstallError::Unknown("x".into()), Some(ErrorCode::Unknown)),
            (InstallError::Cancelled, None),
        ];
        for (err, want) in rows {
            assert_eq!(err.code(), want, "{err}");
        }
    }

    #[test]
    fn user_action_table() {
        assert_eq!(ErrorCode::Offline.user_action(), UserAction::TryAgain);
        assert_eq!(
            ErrorCode::PermissionDenied.user_action(),
            UserAction::TryAgain
        );
        assert_eq!(ErrorCode::Conflicts.user_action(), UserAction::Close);
        assert_eq!(ErrorCode::CorruptFile.user_action(), UserAction::Close);
        assert_eq!(ErrorCode::Unknown.user_action(), UserAction::GetHelp);
        assert_eq!(InstallError::Cancelled.user_action(), None);
    }

    #[test]
    fn backend_maps_to_install_error() {
        assert_eq!(
            InstallError::from(BackendError::Offline),
            InstallError::Offline
        );
        assert_eq!(
            InstallError::from(BackendError::NotAuthorized),
            InstallError::PermissionDenied
        );
        assert_eq!(
            InstallError::from(BackendError::Unavailable("pk".into())),
            InstallError::BackendUnavailable
        );
        assert_eq!(
            InstallError::from(BackendError::Failed("boom".into())).detail(),
            Some("boom")
        );
    }

    #[test]
    fn inspect_maps_to_codes() {
        assert_eq!(InspectError::NotFound.code(), ErrorCode::NotFound);
        assert_eq!(InspectError::Corrupt.code(), ErrorCode::CorruptFile);
        assert_eq!(
            InspectError::Unsupported(UnsupportedReason::Dwarfs).code(),
            ErrorCode::CorruptFile
        );
    }

    #[test]
    fn error_code_wire_names() {
        assert_eq!(
            serde_json::to_string(&ErrorCode::NotEnoughSpace).unwrap(),
            "\"notEnoughSpace\""
        );
    }
}
