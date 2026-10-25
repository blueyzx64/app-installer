//! Install progress and outcomes.

use serde::{Deserialize, Serialize};

use crate::app::InstalledApp;
use crate::error::InstallError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InstallPhase {
    Preparing,
    WaitingForPermission,
    Downloading,
    Installing,
    Finishing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallProgress {
    pub phase: InstallPhase,
    /// 0..=100, or `None` when unknown (indeterminate bar).
    pub percent: Option<u8>,
    pub eta_secs: Option<u32>,
    /// True only while the backend lets the install be cancelled (F-28).
    #[serde(default)]
    pub allow_cancel: bool,
}

impl InstallProgress {
    pub fn phase(phase: InstallPhase) -> Self {
        Self {
            phase,
            percent: None,
            eta_secs: None,
            allow_cancel: false,
        }
    }

    /// Same progress, with cancel allowed or not.
    #[must_use]
    pub fn with_allow_cancel(mut self, allow_cancel: bool) -> Self {
        self.allow_cancel = allow_cancel;
        self
    }

    /// Build from a raw percentage. Anything above 100 (PackageKit sends 101 for
    /// "unknown") becomes `None`.
    pub fn with_percent(phase: InstallPhase, raw_percent: u32, eta_secs: Option<u32>) -> Self {
        let percent = u8::try_from(raw_percent).ok().filter(|p| *p <= 100);
        Self {
            phase,
            percent,
            eta_secs,
            allow_cancel: false,
        }
    }
}

/// What a system install would pull in. Result of a simulated transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct InstallPlan {
    pub extra_packages: u32,
    pub download_bytes: u64,
}

/// What removing an app would do (F-42).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct UninstallPlan {
    /// Other packages that would be removed with it.
    pub also_removes: u32,
    pub needs_permission: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallOutcome {
    Installed(InstalledApp),
    /// The person cancelled (including dismissing the permission dialog). Not an error.
    Cancelled,
    Failed(InstallError),
}

impl InstallOutcome {
    pub fn from_error(e: InstallError) -> Self {
        match e {
            InstallError::Cancelled => InstallOutcome::Cancelled,
            other => InstallOutcome::Failed(other),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UninstallOutcome {
    Removed,
    Cancelled,
    Failed(InstallError),
}

impl UninstallOutcome {
    pub fn from_error(e: InstallError) -> Self {
        match e {
            InstallError::Cancelled => UninstallOutcome::Cancelled,
            other => UninstallOutcome::Failed(other),
        }
    }
}

/// Whether we are the default handler for packages, and whether we asked already.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefaultStatus {
    pub is_default: bool,
    pub asked_before: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_clamps_unknown_to_none() {
        let p = |raw| InstallProgress::with_percent(InstallPhase::Installing, raw, None).percent;
        assert_eq!(p(0), Some(0));
        assert_eq!(p(57), Some(57));
        assert_eq!(p(100), Some(100));
        assert_eq!(p(101), None);
        assert_eq!(p(u32::MAX), None);
    }

    #[test]
    fn cancel_is_not_failure() {
        assert_eq!(
            InstallOutcome::from_error(InstallError::Cancelled),
            InstallOutcome::Cancelled
        );
        assert_eq!(
            InstallOutcome::from_error(InstallError::Offline),
            InstallOutcome::Failed(InstallError::Offline)
        );
        assert_eq!(
            UninstallOutcome::from_error(InstallError::Cancelled),
            UninstallOutcome::Cancelled
        );
    }
}
