use std::sync::Arc;

use installer_domain::{AppId, AppSource, InstallError, UninstallOutcome};

use crate::PreviewRegistry;
use crate::ports::{AppLedger, DesktopIntegration, InstallObserver, SystemPackages, UserApps};

/// Remove an app we installed. Only apps in the ledger can be removed (v1).
///
/// AppImage: move the file to the Trash, remove desktop entry and icons.
/// System package: ask the backend to remove it (polkit prompts there).
pub struct UninstallApp<S, U, D, L> {
    system: Arc<S>,
    user: Arc<U>,
    desktop: Arc<D>,
    ledger: Arc<L>,
    registry: Option<Arc<PreviewRegistry>>,
}

impl<S, U, D, L> UninstallApp<S, U, D, L>
where
    S: SystemPackages,
    U: UserApps,
    D: DesktopIntegration,
    L: AppLedger,
{
    pub fn new(system: Arc<S>, user: Arc<U>, desktop: Arc<D>, ledger: Arc<L>) -> Self {
        Self {
            system,
            user,
            desktop,
            ledger,
            registry: None,
        }
    }

    /// Share the one-at-a-time rule with installs: while an install or another
    /// uninstall runs, `execute` returns `Failed(Busy)`.
    #[must_use]
    pub fn with_registry(mut self, registry: Arc<PreviewRegistry>) -> Self {
        self.registry = Some(registry);
        self
    }

    /// Never returns an error: failures and cancels are outcomes.
    #[tracing::instrument(skip(self, observer), fields(app = %id))]
    pub async fn execute(&self, id: &AppId, observer: &dyn InstallObserver) -> UninstallOutcome {
        let _slot = match self
            .registry
            .as_deref()
            .map(PreviewRegistry::begin_uninstall)
        {
            Some(Err(e)) => return UninstallOutcome::from_error(e),
            Some(Ok(slot)) => Some(slot),
            None => None,
        };
        match self.run(id, observer).await {
            Ok(()) => UninstallOutcome::Removed,
            Err(e) => UninstallOutcome::from_error(e),
        }
    }

    async fn run(&self, id: &AppId, observer: &dyn InstallObserver) -> Result<(), InstallError> {
        let apps = self.ledger.list().await?;
        let app = apps
            .into_iter()
            .find(|a| &a.id == id)
            .ok_or(InstallError::NotFound)?;

        match app.source {
            AppSource::AppImage => {
                match self.user.trash(&app).await {
                    // Deleted by hand already: still clean up what is left.
                    Ok(()) | Err(InstallError::NotFound) => {}
                    Err(e) => return Err(e),
                }
                // The app is already gone; leftovers are cleaned best effort.
                if let Err(e) = self.desktop.remove_entry(&app).await {
                    tracing::warn!(error = %e, "could not remove desktop entry");
                }
            }
            AppSource::SystemPackage => {
                self.system.remove(&app.name, observer).await?;
            }
        }
        if let Err(e) = self.ledger.forget(&app.id).await {
            tracing::warn!(error = %e, "could not forget removed app");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::*;
    use installer_domain::BackendError;

    struct Rig {
        system: Arc<FakeSystemPackages>,
        user: Arc<FakeUserApps>,
        desktop: Arc<FakeDesktop>,
        ledger: Arc<FakeLedger>,
        observer: RecordingObserver,
    }

    impl Rig {
        fn new() -> Self {
            Self {
                system: Arc::new(FakeSystemPackages::default()),
                user: Arc::new(FakeUserApps::default()),
                desktop: Arc::new(FakeDesktop::default()),
                ledger: Arc::new(FakeLedger::default()),
                observer: RecordingObserver::default(),
            }
        }
        fn uc(&self) -> UninstallApp<FakeSystemPackages, FakeUserApps, FakeDesktop, FakeLedger> {
            UninstallApp::new(
                self.system.clone(),
                self.user.clone(),
                self.desktop.clone(),
                self.ledger.clone(),
            )
        }
        async fn run(&self, id: &str) -> UninstallOutcome {
            self.uc()
                .execute(&AppId::from_name(id), &self.observer)
                .await
        }
    }

    #[tokio::test]
    async fn appimage_goes_to_trash_and_entry_removed() {
        let rig = Rig::new();
        rig.ledger
            .seed(sample_app("lumen", "1", AppSource::AppImage));

        assert_eq!(rig.run("lumen").await, UninstallOutcome::Removed);

        assert_eq!(rig.user.trashed().len(), 1);
        assert_eq!(rig.desktop.calls(), ["remove lumen"]);
        assert!(rig.ledger.apps().is_empty());
        assert!(rig.system.calls().is_empty());
    }

    #[tokio::test]
    async fn system_package_is_removed_through_backend() {
        let rig = Rig::new();
        rig.ledger
            .seed(sample_app("lumen", "1", AppSource::SystemPackage));

        assert_eq!(rig.run("lumen").await, UninstallOutcome::Removed);

        assert_eq!(rig.system.calls(), ["remove lumen"]);
        assert!(rig.user.trashed().is_empty());
        assert!(rig.ledger.apps().is_empty());
    }

    #[tokio::test]
    async fn unknown_app_is_not_found() {
        let rig = Rig::new();
        assert_eq!(
            rig.run("nope").await,
            UninstallOutcome::Failed(InstallError::NotFound)
        );
    }

    #[tokio::test]
    async fn dismissed_permission_is_cancel_and_keeps_ledger() {
        let rig = Rig::new();
        rig.ledger
            .seed(sample_app("lumen", "1", AppSource::SystemPackage));
        rig.system.fail_remove(InstallError::Cancelled);

        assert_eq!(rig.run("lumen").await, UninstallOutcome::Cancelled);
        assert_eq!(rig.ledger.apps().len(), 1);
    }

    #[tokio::test]
    async fn backend_failure_keeps_ledger() {
        let rig = Rig::new();
        rig.ledger
            .seed(sample_app("lumen", "1", AppSource::SystemPackage));
        rig.system.fail_remove(InstallError::Conflicts);

        assert_eq!(
            rig.run("lumen").await,
            UninstallOutcome::Failed(InstallError::Conflicts)
        );
        assert_eq!(rig.ledger.apps().len(), 1);
    }

    #[tokio::test]
    async fn trash_failure_keeps_everything() {
        let rig = Rig::new();
        rig.ledger
            .seed(sample_app("lumen", "1", AppSource::AppImage));
        rig.user.fail_trash(InstallError::PermissionDenied);

        assert_eq!(
            rig.run("lumen").await,
            UninstallOutcome::Failed(InstallError::PermissionDenied)
        );
        assert!(
            rig.desktop.calls().is_empty(),
            "entry kept while the file is still there"
        );
        assert_eq!(rig.ledger.apps().len(), 1);
    }

    #[tokio::test]
    async fn leftover_cleanup_failures_do_not_fail_removal() {
        let rig = Rig::new();
        rig.ledger
            .seed(sample_app("lumen", "1", AppSource::AppImage));
        rig.desktop.fail_remove(InstallError::Unknown("x".into()));
        assert_eq!(rig.run("lumen").await, UninstallOutcome::Removed);
    }

    #[tokio::test]
    async fn unreadable_ledger_fails() {
        let rig = Rig::new();
        rig.ledger.fail(BackendError::Io("corrupt".into()));
        assert_eq!(
            rig.run("lumen").await,
            UninstallOutcome::Failed(InstallError::Unknown("corrupt".into()))
        );
    }

    #[tokio::test]
    async fn uninstall_is_busy_while_an_install_runs_and_frees_its_slot() {
        let rig = Rig::new();
        rig.ledger.seed(sample_app(
            "lumen",
            "1",
            installer_domain::AppSource::AppImage,
        ));
        let registry = Arc::new(PreviewRegistry::new());
        let file = sample_file(installer_domain::PackageFormat::AppImage);
        let preview = registry.insert(file.clone(), |id| installer_domain::PackagePreview {
            id,
            size_bytes: file.size_bytes,
            meta: sample_meta(installer_domain::PackageFormat::AppImage),
            compat: installer_domain::Compatibility::Ok,
            needs_permission: false,
            installed_app_id: None,
        });
        let uc = rig.uc().with_registry(registry.clone());
        let id = AppId::from_name("lumen");

        let slot = registry.begin_guarded(preview.id).unwrap();
        assert_eq!(
            uc.execute(&id, &rig.observer).await,
            UninstallOutcome::Failed(InstallError::Busy)
        );
        assert!(rig.user.trashed().is_empty(), "nothing touched while busy");
        drop(slot);
        assert_eq!(
            uc.execute(&id, &rig.observer).await,
            UninstallOutcome::Removed
        );
        assert!(
            registry.begin_uninstall().is_ok(),
            "slot freed after uninstall"
        );
    }
}
