//! Small shell-side adapters: the "unavailable" system backend (D-020) and the
//! per-format inspector chain. No IO, no `tauri`.

use std::time::Duration;

use installer_application::CancelToken;
use installer_application::ports::{InstallObserver, PackageInspector, SystemPackages};
use installer_domain::{
    BackendError, InspectError, InstallError, InstallPlan, PackageFile, PackageFormat,
    PackageMetadata, UnsupportedReason, Version,
};
#[cfg(feature = "dnf5daemon")]
use installer_infrastructure::dnf5daemon::{Dnf5Daemon, ZbusBus};
use installer_infrastructure::packagekit::PackageKitSystemPackages;

/// System packages until the PackageKit backend lands (W2, D-020).
/// Nothing is installed, and installing says the backend is unavailable.
#[derive(Debug, Default, Clone, Copy)]
pub struct UnavailableSystemPackages;

impl SystemPackages for UnavailableSystemPackages {
    async fn installed_version(&self, _name: &str) -> Result<Option<Version>, BackendError> {
        Ok(None)
    }

    async fn plan(&self, _file: &PackageFile) -> Result<InstallPlan, BackendError> {
        Err(BackendError::Unavailable("system backend".into()))
    }

    async fn install(
        &self,
        _file: &PackageFile,
        _observer: &dyn InstallObserver,
        _cancel: CancelToken,
    ) -> Result<(), InstallError> {
        Err(InstallError::BackendUnavailable)
    }

    async fn remove(
        &self,
        _name: &str,
        _observer: &dyn InstallObserver,
    ) -> Result<(), InstallError> {
        Err(InstallError::BackendUnavailable)
    }

    async fn removal_plan(&self, _name: &str) -> Result<u32, BackendError> {
        Err(BackendError::Unavailable("system backend".into()))
    }
}

/// The system backend chosen at startup: PackageKit, else dnf5daemon, else nothing.
pub enum AnySystemPackages {
    PackageKit(PackageKitSystemPackages),
    #[cfg(feature = "dnf5daemon")]
    Dnf5(Dnf5Daemon<ZbusBus>),
    Unavailable(UnavailableSystemPackages),
}

impl AnySystemPackages {
    /// Pick a backend at startup. Each backend gets `STEP_BUDGET` to answer a
    /// cheap health call; a hung or broken one is skipped. Worst case is about
    /// 1.4 s in total, so a wedged bus can never stop the window from opening.
    pub async fn detect() -> Self {
        let pk = PackageKitSystemPackages::new();
        if healthy("PackageKit", async {
            pk.is_available().await && pk.installed_version(PROBE_NAME).await.is_ok()
        })
        .await
        {
            return Self::PackageKit(pk);
        }
        #[cfg(feature = "dnf5daemon")]
        {
            let picked = tokio::time::timeout(STEP_BUDGET, async {
                if !installer_infrastructure::dnf5daemon::is_available().await {
                    return None;
                }
                let d = Dnf5Daemon::connect().await.ok()?;
                d.installed_version(PROBE_NAME).await.ok()?;
                Some(d)
            })
            .await;
            match picked {
                Ok(Some(d)) => return Self::Dnf5(d),
                Ok(None) => {}
                Err(_) => log::warn!("dnf5daemon did not answer in time"),
            }
        }
        Self::Unavailable(UnavailableSystemPackages)
    }
}

/// Longest we wait for one backend during startup.
const STEP_BUDGET: Duration = Duration::from_millis(700);
/// A name no package has: the probe is a read-only lookup that finds nothing.
const PROBE_NAME: &str = "installer-backend-probe";

async fn healthy(what: &str, probe: impl std::future::Future<Output = bool>) -> bool {
    match tokio::time::timeout(STEP_BUDGET, probe).await {
        Ok(ok) => ok,
        Err(_) => {
            log::warn!("{what} did not answer in time");
            false
        }
    }
}

impl SystemPackages for AnySystemPackages {
    async fn installed_version(&self, name: &str) -> Result<Option<Version>, BackendError> {
        match self {
            Self::PackageKit(s) => s.installed_version(name).await,
            #[cfg(feature = "dnf5daemon")]
            Self::Dnf5(s) => s.installed_version(name).await,
            Self::Unavailable(s) => s.installed_version(name).await,
        }
    }

    async fn plan(&self, file: &PackageFile) -> Result<InstallPlan, BackendError> {
        match self {
            Self::PackageKit(s) => s.plan(file).await,
            #[cfg(feature = "dnf5daemon")]
            Self::Dnf5(s) => s.plan(file).await,
            Self::Unavailable(s) => s.plan(file).await,
        }
    }

    async fn install(
        &self,
        file: &PackageFile,
        observer: &dyn InstallObserver,
        cancel: CancelToken,
    ) -> Result<(), InstallError> {
        match self {
            Self::PackageKit(s) => s.install(file, observer, cancel).await,
            #[cfg(feature = "dnf5daemon")]
            Self::Dnf5(s) => s.install(file, observer, cancel).await,
            Self::Unavailable(s) => s.install(file, observer, cancel).await,
        }
    }

    async fn remove(&self, name: &str, observer: &dyn InstallObserver) -> Result<(), InstallError> {
        match self {
            Self::PackageKit(s) => s.remove(name, observer).await,
            #[cfg(feature = "dnf5daemon")]
            Self::Dnf5(s) => s.remove(name, observer).await,
            Self::Unavailable(s) => s.remove(name, observer).await,
        }
    }

    async fn removal_plan(&self, name: &str) -> Result<u32, BackendError> {
        match self {
            Self::PackageKit(s) => s.removal_plan(name).await,
            #[cfg(feature = "dnf5daemon")]
            Self::Dnf5(s) => s.removal_plan(name).await,
            Self::Unavailable(s) => s.removal_plan(name).await,
        }
    }
}

/// Supports no format. End of an [`InspectorChain`].
#[derive(Debug, Default, Clone, Copy)]
pub struct NoInspector;

impl PackageInspector for NoInspector {
    fn supports(&self, _format: PackageFormat) -> bool {
        false
    }

    async fn inspect(&self, _file: &PackageFile) -> Result<PackageMetadata, InspectError> {
        Err(InspectError::Unsupported(UnsupportedReason::UnknownFormat))
    }
}

/// Dispatches by format: `head` first, otherwise `tail`.
/// Build with `InspectorChain::new(appimage, InspectorChain::new(rpm, NoInspector))`.
#[derive(Debug, Clone)]
pub struct InspectorChain<H, T> {
    head: H,
    tail: T,
}

impl<H, T> InspectorChain<H, T> {
    pub fn new(head: H, tail: T) -> Self {
        Self { head, tail }
    }
}

impl<H: PackageInspector, T: PackageInspector> PackageInspector for InspectorChain<H, T> {
    fn supports(&self, format: PackageFormat) -> bool {
        self.head.supports(format) || self.tail.supports(format)
    }

    async fn inspect(&self, file: &PackageFile) -> Result<PackageMetadata, InspectError> {
        if self.head.supports(file.format) {
            self.head.inspect(file).await
        } else {
            self.tail.inspect(file).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use installer_application::testing::{FakeInspector, RecordingObserver};
    use std::path::PathBuf;

    fn file(format: PackageFormat) -> PackageFile {
        PackageFile {
            path: PathBuf::from("/x"),
            format,
            size_bytes: 1,
        }
    }

    #[tokio::test]
    async fn unavailable_system_reports_unavailable() {
        let s = UnavailableSystemPackages;
        assert_eq!(s.installed_version("x").await, Ok(None));
        let r = s
            .install(
                &file(PackageFormat::Rpm),
                &RecordingObserver::default(),
                CancelToken::new(),
            )
            .await;
        assert_eq!(r, Err(InstallError::BackendUnavailable));
    }

    #[tokio::test]
    async fn chain_routes_by_format() {
        let app = FakeInspector::default();
        app.only_formats(&[PackageFormat::AppImage]);
        let chain = InspectorChain::new(app, NoInspector);
        assert!(chain.supports(PackageFormat::AppImage));
        assert!(!chain.supports(PackageFormat::Rpm));
        assert!(matches!(
            chain.inspect(&file(PackageFormat::Rpm)).await,
            Err(InspectError::Unsupported(_))
        ));
    }
}
