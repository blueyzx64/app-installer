//! `SystemPackages` over dnf5daemon (`org.rpm.dnf.v0`, system bus). F-31.
//!
//! Fallback for hosts where PackageKit is missing (ADR-0004). Layout like
//! `packagekit/`: `bus.rs` is the seam (trait + plain data), `proxy.rs` is the
//! zbus implementation, `mapping.rs` is pure translation, this file is the flow.
//!
//! Flow per operation (one session each, always closed):
//! `open_session` -> queue spec -> `Goal.resolve` (plan, F-27) -> subscribe
//! signals -> `Goal.do_transaction` (polkit `auth_admin`) -> `close_session`.
//! Phases follow D-019: Preparing -> WaitingForPermission -> Downloading ->
//! Installing -> Finishing.
//!
//! ⚠ Every interface/method name in `proxy.rs` is from memory of
//! `dnf5daemon-dbus-api(8)`; HUMAN must verify on Fedora 45.
//! Never shells out to dnf/rpm/pkexec (CLAUDE.md rule 4).

mod bus;
mod mapping;
mod proxy;

use installer_application::CancelToken;
use installer_application::ports::{InstallObserver, SystemPackages};
use installer_domain::{
    BackendError, InstallError, InstallPhase, InstallPlan, InstallProgress, PackageFile, Version,
};

pub use bus::{BusError, Dnf5Bus, Dnf5Signal, InstalledRpm, Resolution, ResolvedItem, SessionId};
pub use proxy::{ZbusBus, is_available};

use mapping::{
    ProgressTracker, bus_to_backend, bus_to_install, installed_version, plan_from,
    problems_to_error, removal_extra,
};

/// dnf5daemon-backed `SystemPackages`.
pub struct Dnf5Daemon<B: Dnf5Bus> {
    bus: B,
}

impl Dnf5Daemon<ZbusBus> {
    /// Connect to the system bus. Fails with `Unavailable` when there is none.
    pub async fn connect() -> Result<Self, BackendError> {
        Ok(Self::new(ZbusBus::system().await.map_err(bus_to_backend)?))
    }
}

impl<B: Dnf5Bus> Dnf5Daemon<B> {
    pub fn new(bus: B) -> Self {
        Self { bus }
    }

    /// Backs `SystemPackages::removal_plan`: extra packages a removal takes along.
    async fn plan_removal(&self, name: &str) -> Result<u32, BackendError> {
        let sid = self.bus.open_session().await.map_err(bus_to_backend)?;
        let r = async {
            self.bus.queue_remove(&sid, name).await?;
            self.bus.resolve(&sid).await
        }
        .await;
        let _ = self.bus.close_session(&sid).await;
        let res = r.map_err(bus_to_backend)?;
        if !res.problems.is_empty() {
            return Err(BackendError::Failed(res.problems.join("; ")));
        }
        Ok(removal_extra(&res))
    }

    async fn run_goal(
        &self,
        sid: &SessionId,
        spec: Spec<'_>,
        observer: &dyn InstallObserver,
        cancel: &CancelToken,
    ) -> Result<(), InstallError> {
        let mut tracker = ProgressTracker::new();
        let emit = |p: Option<InstallProgress>| {
            if let Some(p) = p {
                observer.on_progress(p);
            }
        };
        emit(tracker.enter(InstallPhase::Preparing));

        match spec {
            Spec::Install(path) => self.bus.queue_install(sid, path).await,
            Spec::Remove(name) => self.bus.queue_remove(sid, name).await,
        }
        .map_err(bus_to_install)?;
        let res = self.bus.resolve(sid).await.map_err(bus_to_install)?;
        if let Some(e) = problems_to_error(&res.problems) {
            return Err(e);
        }
        if matches!(spec, Spec::Install(_)) {
            observer.on_plan(&plan_from(&res));
        }
        if cancel.is_cancelled() {
            return Err(InstallError::Cancelled);
        }

        // Subscribe BEFORE the transaction so no early signal is lost.
        let mut rx = self.bus.subscribe(sid).await.map_err(bus_to_install)?;
        // The polkit dialog appears inside do_transaction.
        emit(tracker.enter(InstallPhase::WaitingForPermission));

        let tx = self.bus.do_transaction(sid);
        tokio::pin!(tx);
        let mut cancelled = false;
        let result = loop {
            tokio::select! {
                r = &mut tx => break r,
                Some(sig) = rx.recv() => emit(tracker.on_signal(&sig)),
                () = cancel.cancelled(), if !cancelled => {
                    cancelled = true;
                    // Best effort. Goal.cancel only works once the goal runs.
                    let _ = self.bus.cancel(sid).await;
                }
            }
        };
        while let Ok(sig) = rx.try_recv() {
            emit(tracker.on_signal(&sig));
        }
        match result {
            Ok(()) => {
                emit(tracker.enter(InstallPhase::Finishing));
                Ok(())
            }
            Err(_) if cancelled => Err(InstallError::Cancelled),
            Err(e) => Err(bus_to_install(e)),
        }
    }

    async fn with_session<T>(
        &self,
        f: impl AsyncFnOnce(&SessionId) -> Result<T, InstallError>,
    ) -> Result<T, InstallError> {
        let sid = self.bus.open_session().await.map_err(bus_to_install)?;
        let r = f(&sid).await;
        let _ = self.bus.close_session(&sid).await;
        r
    }
}

#[derive(Clone, Copy)]
enum Spec<'a> {
    Install(&'a str),
    Remove(&'a str),
}

impl<B: Dnf5Bus> SystemPackages for Dnf5Daemon<B> {
    async fn installed_version(&self, name: &str) -> Result<Option<Version>, BackendError> {
        let sid = self.bus.open_session().await.map_err(bus_to_backend)?;
        let r = self.bus.list_installed(&sid, name).await;
        let _ = self.bus.close_session(&sid).await;
        Ok(installed_version(&r.map_err(bus_to_backend)?))
    }

    async fn plan(&self, file: &PackageFile) -> Result<InstallPlan, BackendError> {
        let path = file.path.to_string_lossy().into_owned();
        let sid = self.bus.open_session().await.map_err(bus_to_backend)?;
        let r = async {
            self.bus.queue_install(&sid, &path).await?;
            self.bus.resolve(&sid).await
        }
        .await;
        let _ = self.bus.close_session(&sid).await;
        let res = r.map_err(bus_to_backend)?;
        if !res.problems.is_empty() {
            return Err(BackendError::Failed(res.problems.join("; ")));
        }
        Ok(plan_from(&res))
    }

    async fn install(
        &self,
        file: &PackageFile,
        observer: &dyn InstallObserver,
        cancel: CancelToken,
    ) -> Result<(), InstallError> {
        let path = file.path.to_string_lossy().into_owned();
        self.with_session(async |sid| {
            self.run_goal(sid, Spec::Install(&path), observer, &cancel)
                .await
        })
        .await
    }

    async fn remove(&self, name: &str, observer: &dyn InstallObserver) -> Result<(), InstallError> {
        let cancel = CancelToken::new();
        self.with_session(async |sid| {
            self.run_goal(sid, Spec::Remove(name), observer, &cancel)
                .await
        })
        .await
    }

    async fn removal_plan(&self, name: &str) -> Result<u32, BackendError> {
        self.plan_removal(name).await
    }
}

#[cfg(test)]
mod tests;
