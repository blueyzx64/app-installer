use installer_domain::{AppId, BackendError, InstalledApp};

/// Record of the apps we installed (a JSON file in the real adapter).
#[trait_variant::make(Send)]
pub trait AppLedger: Sync {
    async fn record(&self, app: &InstalledApp) -> Result<(), BackendError>;
    async fn forget(&self, id: &AppId) -> Result<(), BackendError>;
    async fn list(&self) -> Result<Vec<InstalledApp>, BackendError>;
}
