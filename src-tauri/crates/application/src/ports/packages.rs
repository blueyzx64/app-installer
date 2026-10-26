use std::path::PathBuf;

use installer_domain::{
    BackendError, InstallError, InstallPlan, InstallProgress, InstalledApp, PackageFile,
    PackageMetadata, Version,
};

use crate::CancelToken;

/// Receives progress. Sync and object safe so adapters can forward to a channel.
pub trait InstallObserver: Send + Sync {
    fn on_progress(&self, progress: InstallProgress);

    /// What a system install will pull in. Optional.
    fn on_plan(&self, _plan: &InstallPlan) {}
}

/// System-wide packages: PackageKit (primary) or dnf5daemon (fallback).
#[trait_variant::make(Send)]
pub trait SystemPackages: Sync {
    async fn installed_version(&self, name: &str) -> Result<Option<Version>, BackendError>;

    /// Simulate the install and report what it would pull in.
    async fn plan(&self, file: &PackageFile) -> Result<InstallPlan, BackendError>;

    /// Install. A dismissed permission dialog or a cancel must return
    /// `InstallError::Cancelled`, never a failure.
    async fn install(
        &self,
        file: &PackageFile,
        observer: &dyn InstallObserver,
        cancel: CancelToken,
    ) -> Result<(), InstallError>;

    /// Simulate the removal. Returns how many other packages would go with it.
    async fn removal_plan(&self, name: &str) -> Result<u32, BackendError>;

    async fn remove(&self, name: &str, observer: &dyn InstallObserver) -> Result<(), InstallError>;
}

/// Per-user apps: the AppImage store in `~/Applications`.
#[trait_variant::make(Send)]
pub trait UserApps: Sync {
    /// Copy the file into the store. Returns the final path.
    async fn install(
        &self,
        file: &PackageFile,
        meta: &PackageMetadata,
    ) -> Result<PathBuf, InstallError>;

    /// Move the app file to the Trash (recoverable).
    async fn trash(&self, app: &InstalledApp) -> Result<(), InstallError>;

    /// Every app file in the store, with its path (canonical), identity key (`name`) and
    /// version read from the file. Ids are provisional: the ledger owns ids and matches
    /// entries to files by path (D-037). A file that exists but cannot be read is listed
    /// with an empty `name` and version, so callers keep what they know about it.
    /// `Err` only when the store itself cannot be read; a missing store is empty.
    async fn scan(&self) -> Result<Vec<InstalledApp>, InstallError>;
}
