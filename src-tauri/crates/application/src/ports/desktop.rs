use installer_domain::{BackendError, DefaultStatus, IconData, InstallError, InstalledApp};

/// Desktop files and icons for per-user apps. System packages ship their own.
#[trait_variant::make(Send)]
pub trait DesktopIntegration: Sync {
    /// Write the generated `.desktop` entry and icons. Returns the desktop file id.
    async fn install_entry(
        &self,
        app: &InstalledApp,
        icon: Option<&IconData>,
    ) -> Result<String, InstallError>;

    /// Remove the entry and icons. Missing files are not an error.
    async fn remove_entry(&self, app: &InstalledApp) -> Result<(), InstallError>;
}

#[trait_variant::make(Send)]
pub trait Launcher: Sync {
    async fn launch(&self, app: &InstalledApp) -> Result<(), BackendError>;
}

/// Default handler for package MIME types, plus the "Not Now" memory.
#[trait_variant::make(Send)]
pub trait DefaultHandlerRegistry: Sync {
    async fn status(&self) -> Result<DefaultStatus, BackendError>;
    async fn make_default(&self) -> Result<(), BackendError>;
    /// Remember that the person said "Not Now".
    async fn dismiss_prompt(&self) -> Result<(), BackendError>;
}
