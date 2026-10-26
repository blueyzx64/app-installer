//! Which system package backend to use (D-040).
//!
//! Startup only runs cheap liveness checks, each with a timeout: the bus name
//! has an owner or is activatable, plus one `Properties.Get(VersionMajor)` for
//! PackageKit. No package lookup and no dnf5daemon session at startup, so a
//! slow repo load can never get a healthy daemon rejected and no probe session
//! can leak. If PackageKit then answers its first real call with "backend
//! unavailable", we switch once to dnf5daemon and retry that call.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use installer_application::CancelToken;
use installer_application::ports::{InstallObserver, SystemPackages};
use installer_domain::{BackendError, InstallError, InstallPlan, PackageFile, Version};
use installer_infrastructure::packagekit::PackageKitSystemPackages;
use tokio::sync::OnceCell;

use crate::fallbacks::UnavailableSystemPackages;

/// Longest we wait for one liveness check during startup.
const STEP_BUDGET: Duration = Duration::from_millis(700);

const PK_SERVICE: &str = "org.freedesktop.PackageKit";
const PK_PATH: &str = "/org/freedesktop/PackageKit";

#[cfg(feature = "dnf5daemon")]
mod next {
    pub use installer_infrastructure::dnf5daemon::{Dnf5Daemon, ZbusBus};
    pub type Next = Dnf5Daemon<ZbusBus>;

    /// Has an owner or is activatable, then a plain bus connection. No session.
    pub async fn alive() -> Option<Next> {
        if !installer_infrastructure::dnf5daemon::is_available().await {
            return None;
        }
        Dnf5Daemon::connect().await.ok()
    }
}

#[cfg(not(feature = "dnf5daemon"))]
mod next {
    pub type Next = crate::fallbacks::UnavailableSystemPackages;
    pub async fn alive() -> Option<Next> {
        None
    }
}

use next::Next;

/// The system backend chosen at startup.
pub enum AnySystemPackages {
    PackageKit(PackageKitWithFallback),
    Dnf5(Next),
    Unavailable(UnavailableSystemPackages),
}

impl AnySystemPackages {
    /// Worst case about 1.4 s (two budgets), so a wedged bus never blocks startup.
    pub async fn detect() -> Self {
        let pk = PackageKitSystemPackages::new();
        if within("PackageKit", async {
            pk.is_available().await && pk_version_ok().await
        })
        .await
        .unwrap_or(false)
        {
            return Self::PackageKit(PackageKitWithFallback::new(pk));
        }
        if let Some(Some(d)) = within("dnf5daemon", next::alive()).await {
            return Self::Dnf5(d);
        }
        Self::Unavailable(UnavailableSystemPackages)
    }
}

/// Run `fut` with the startup budget. A timeout drops it, which closes nothing
/// because no probe opens a session.
async fn within<T>(what: &str, fut: impl Future<Output = T>) -> Option<T> {
    match tokio::time::timeout(STEP_BUDGET, fut).await {
        Ok(v) => Some(v),
        Err(_) => {
            log::warn!("{what} did not answer in time");
            None
        }
    }
}

/// One `Properties.Get(org.freedesktop.PackageKit, VersionMajor)`.
async fn pk_version_ok() -> bool {
    async fn get() -> zbus::Result<()> {
        let conn = zbus::Connection::system().await?;
        let props = zbus::fdo::PropertiesProxy::builder(&conn)
            .destination(PK_SERVICE)?
            .path(PK_PATH)?
            .build()
            .await?;
        let iface = zbus::names::InterfaceName::try_from(PK_SERVICE)?;
        props.get(iface, "VersionMajor").await?;
        Ok(())
    }
    get().await.is_ok()
}

/// PackageKit, with a one-time lazy switch to dnf5daemon.
pub struct PackageKitWithFallback {
    pk: PackageKitSystemPackages,
    next: OnceCell<Option<Next>>,
    switched: AtomicBool,
}

impl PackageKitWithFallback {
    pub fn new(pk: PackageKitSystemPackages) -> Self {
        Self {
            pk,
            next: OnceCell::new(),
            switched: AtomicBool::new(false),
        }
    }

    /// The fallback, once we have switched to it.
    fn active_next(&self) -> Option<&Next> {
        if self.switched.load(Ordering::Acquire) {
            self.next.get().and_then(Option::as_ref)
        } else {
            None
        }
    }

    /// Switch (once) after PackageKit said it is unavailable.
    async fn switch(&self) -> Option<&Next> {
        let next = self
            .next
            .get_or_init(|| async { within("dnf5daemon", next::alive()).await.flatten() })
            .await
            .as_ref()?;
        if !self.switched.swap(true, Ordering::AcqRel) {
            log::warn!("PackageKit is unavailable; switching to dnf5daemon");
        }
        Some(next)
    }
}

fn backend_unavailable(e: &BackendError) -> bool {
    matches!(e, BackendError::Unavailable(_))
}

impl SystemPackages for PackageKitWithFallback {
    async fn installed_version(&self, name: &str) -> Result<Option<Version>, BackendError> {
        if let Some(n) = self.active_next() {
            return n.installed_version(name).await;
        }
        match self.pk.installed_version(name).await {
            Err(e) if backend_unavailable(&e) => match self.switch().await {
                Some(n) => n.installed_version(name).await,
                None => Err(e),
            },
            r => r,
        }
    }

    async fn plan(&self, file: &PackageFile) -> Result<InstallPlan, BackendError> {
        if let Some(n) = self.active_next() {
            return n.plan(file).await;
        }
        match self.pk.plan(file).await {
            Err(e) if backend_unavailable(&e) => match self.switch().await {
                Some(n) => n.plan(file).await,
                None => Err(e),
            },
            r => r,
        }
    }

    async fn install(
        &self,
        file: &PackageFile,
        observer: &dyn InstallObserver,
        cancel: CancelToken,
    ) -> Result<(), InstallError> {
        if let Some(n) = self.active_next() {
            return n.install(file, observer, cancel).await;
        }
        match self.pk.install(file, observer, cancel.clone()).await {
            Err(InstallError::BackendUnavailable) => match self.switch().await {
                Some(n) => n.install(file, observer, cancel).await,
                None => Err(InstallError::BackendUnavailable),
            },
            r => r,
        }
    }

    async fn remove(&self, name: &str, observer: &dyn InstallObserver) -> Result<(), InstallError> {
        if let Some(n) = self.active_next() {
            return n.remove(name, observer).await;
        }
        match self.pk.remove(name, observer).await {
            Err(InstallError::BackendUnavailable) => match self.switch().await {
                Some(n) => n.remove(name, observer).await,
                None => Err(InstallError::BackendUnavailable),
            },
            r => r,
        }
    }

    async fn removal_plan(&self, name: &str) -> Result<u32, BackendError> {
        if let Some(n) = self.active_next() {
            return n.removal_plan(name).await;
        }
        match self.pk.removal_plan(name).await {
            Err(e) if backend_unavailable(&e) => match self.switch().await {
                Some(n) => n.removal_plan(name).await,
                None => Err(e),
            },
            r => r,
        }
    }
}

impl SystemPackages for AnySystemPackages {
    async fn installed_version(&self, name: &str) -> Result<Option<Version>, BackendError> {
        match self {
            Self::PackageKit(s) => s.installed_version(name).await,
            Self::Dnf5(s) => s.installed_version(name).await,
            Self::Unavailable(s) => s.installed_version(name).await,
        }
    }

    async fn plan(&self, file: &PackageFile) -> Result<InstallPlan, BackendError> {
        match self {
            Self::PackageKit(s) => s.plan(file).await,
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
            Self::Dnf5(s) => s.install(file, observer, cancel).await,
            Self::Unavailable(s) => s.install(file, observer, cancel).await,
        }
    }

    async fn remove(&self, name: &str, observer: &dyn InstallObserver) -> Result<(), InstallError> {
        match self {
            Self::PackageKit(s) => s.remove(name, observer).await,
            Self::Dnf5(s) => s.remove(name, observer).await,
            Self::Unavailable(s) => s.remove(name, observer).await,
        }
    }

    async fn removal_plan(&self, name: &str) -> Result<u32, BackendError> {
        match self {
            Self::PackageKit(s) => s.removal_plan(name).await,
            Self::Dnf5(s) => s.removal_plan(name).await,
            Self::Unavailable(s) => s.removal_plan(name).await,
        }
    }
}
