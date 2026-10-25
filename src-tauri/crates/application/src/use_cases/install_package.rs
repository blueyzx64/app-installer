use std::sync::Arc;

use installer_domain::{
    AppId, AppSource, InstallError, InstallOutcome, InstallPhase, InstallProgress, InstallScope,
    InstalledApp, PreviewId,
};

use crate::ports::{
    AppLedger, Clock, DesktopIntegration, InstallObserver, SystemPackages, UserApps,
};
use crate::registry::StoredPreview;
use crate::{CancelToken, PreviewRegistry};

/// Install a previously opened package.
///
/// System scope goes through `SystemPackages` (polkit asks for permission there).
/// User scope copies into the app store and writes the desktop entry.
pub struct InstallPackage<S, U, D, L, C> {
    system: Arc<S>,
    user: Arc<U>,
    desktop: Arc<D>,
    ledger: Arc<L>,
    clock: Arc<C>,
    registry: Arc<PreviewRegistry>,
}

impl<S, U, D, L, C> InstallPackage<S, U, D, L, C>
where
    S: SystemPackages,
    U: UserApps,
    D: DesktopIntegration,
    L: AppLedger,
    C: Clock,
{
    pub fn new(
        system: Arc<S>,
        user: Arc<U>,
        desktop: Arc<D>,
        ledger: Arc<L>,
        clock: Arc<C>,
        registry: Arc<PreviewRegistry>,
    ) -> Self {
        Self {
            system,
            user,
            desktop,
            ledger,
            clock,
            registry,
        }
    }

    /// Never returns an error: failures and cancels are outcomes.
    /// On success the preview is dropped; otherwise it stays so the person can retry.
    #[tracing::instrument(skip_all, fields(preview = %id))]
    pub async fn execute(&self, id: PreviewId, observer: &dyn InstallObserver) -> InstallOutcome {
        let (stored, cancel) = match self.registry.begin(id) {
            Ok(x) => x,
            Err(e) => return InstallOutcome::from_error(e),
        };
        let result = self.run(&stored, observer, &cancel).await;
        self.registry.finish(id, result.is_err());
        match result {
            Ok(app) => InstallOutcome::Installed(app),
            Err(e) => InstallOutcome::from_error(e),
        }
    }

    async fn run(
        &self,
        stored: &StoredPreview,
        observer: &dyn InstallObserver,
        cancel: &CancelToken,
    ) -> Result<InstalledApp, InstallError> {
        let StoredPreview { file, preview } = stored;
        let meta = &preview.meta;
        if !preview.compat.is_installable() {
            return Err(InstallError::Unknown("package is not installable".into()));
        }
        observer.on_progress(InstallProgress::phase(InstallPhase::Preparing));
        check_cancel(cancel)?;

        let mut app = InstalledApp {
            id: AppId::from_name(&meta.name),
            name: meta.name.clone(),
            display_name: meta.display_name.clone(),
            version: meta.version.clone(),
            icon: meta.icon.clone(),
            source: match meta.scope {
                InstallScope::System => AppSource::SystemPackage,
                InstallScope::User => AppSource::AppImage,
            },
            installed_at: self.clock.now_unix(),
            desktop_id: None,
            path: None,
        };

        match meta.scope {
            InstallScope::System => {
                let plan = self.system.plan(file).await?;
                observer.on_plan(&plan);
                check_cancel(cancel)?;
                self.system.install(file, observer, cancel.clone()).await?;
            }
            InstallScope::User => {
                observer.on_progress(InstallProgress::phase(InstallPhase::Installing));
                app.path = Some(self.user.install(file, meta).await?);
                match self.desktop.install_entry(&app, meta.icon.as_ref()).await {
                    Ok(desktop_id) => app.desktop_id = Some(desktop_id),
                    Err(e) => {
                        // Do not leave a half-installed app behind.
                        if let Err(cleanup) = self.user.trash(&app).await {
                            tracing::warn!(error = %cleanup, "cleanup after failed install failed");
                        }
                        return Err(e);
                    }
                }
            }
        }

        observer.on_progress(InstallProgress::phase(InstallPhase::Finishing));
        // The app is installed even if we cannot write the ledger; listing reconciles later.
        if let Err(e) = self.ledger.record(&app).await {
            tracing::warn!(error = %e, "could not record installed app");
        }
        Ok(app)
    }
}

fn check_cancel(cancel: &CancelToken) -> Result<(), InstallError> {
    if cancel.is_cancelled() {
        Err(InstallError::Cancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::*;
    use installer_domain::{
        BackendError, Compatibility, InstallPlan, PackageFormat, PackagePreview, Version,
        VersionRelation,
    };

    struct Rig {
        system: Arc<FakeSystemPackages>,
        user: Arc<FakeUserApps>,
        desktop: Arc<FakeDesktop>,
        ledger: Arc<FakeLedger>,
        registry: Arc<PreviewRegistry>,
        observer: RecordingObserver,
    }

    impl Rig {
        fn new() -> Self {
            Self {
                system: Arc::new(FakeSystemPackages::default()),
                user: Arc::new(FakeUserApps::default()),
                desktop: Arc::new(FakeDesktop::default()),
                ledger: Arc::new(FakeLedger::default()),
                registry: Arc::new(PreviewRegistry::new()),
                observer: RecordingObserver::default(),
            }
        }

        fn uc(
            &self,
        ) -> InstallPackage<FakeSystemPackages, FakeUserApps, FakeDesktop, FakeLedger, FakeClock>
        {
            InstallPackage::new(
                self.system.clone(),
                self.user.clone(),
                self.desktop.clone(),
                self.ledger.clone(),
                Arc::new(FakeClock(1_700_000_000)),
                self.registry.clone(),
            )
        }

        fn preview(&self, format: PackageFormat, compat: Compatibility) -> PackagePreview {
            let meta = sample_meta(format);
            let file = sample_file(format);
            let scope = meta.scope;
            self.registry.insert(file.clone(), |id| PackagePreview {
                id,
                size_bytes: file.size_bytes,
                meta,
                compat,
                needs_permission: scope.needs_permission(),
            })
        }
    }

    #[tokio::test]
    async fn system_install_happy_path() {
        let rig = Rig::new();
        rig.system.set_plan(InstallPlan {
            extra_packages: 2,
            download_bytes: 5_000,
        });
        let p = rig.preview(PackageFormat::Rpm, Compatibility::Ok);

        let out = rig.uc().execute(p.id, &rig.observer).await;

        let InstallOutcome::Installed(app) = out else {
            panic!("expected installed, got {out:?}");
        };
        assert_eq!(app.id.as_str(), "lumen-notes");
        assert_eq!(app.source, AppSource::SystemPackage);
        assert_eq!(app.installed_at, 1_700_000_000);
        assert_eq!(rig.ledger.apps().len(), 1);
        assert_eq!(
            rig.observer.plans(),
            vec![InstallPlan {
                extra_packages: 2,
                download_bytes: 5_000
            }]
        );
        let phases = rig.observer.phases();
        assert_eq!(phases.first(), Some(&InstallPhase::Preparing));
        assert!(phases.contains(&InstallPhase::WaitingForPermission));
        assert_eq!(phases.last(), Some(&InstallPhase::Finishing));
        assert!(
            rig.registry.get(p.id).is_none(),
            "preview dropped on success"
        );
    }

    #[tokio::test]
    async fn user_install_writes_entry_and_records() {
        let rig = Rig::new();
        let p = rig.preview(PackageFormat::AppImage, Compatibility::Ok);

        let out = rig.uc().execute(p.id, &rig.observer).await;

        let InstallOutcome::Installed(app) = out else {
            panic!("expected installed, got {out:?}");
        };
        assert_eq!(app.source, AppSource::AppImage);
        assert_eq!(
            app.path.as_deref(),
            Some(std::path::Path::new(
                "/home/u/Applications/Lumen Notes.AppImage"
            ))
        );
        assert_eq!(
            app.desktop_id.as_deref(),
            Some("io.installer.lumen-notes.desktop")
        );
        assert_eq!(rig.ledger.apps(), vec![app]);
        assert!(
            rig.system.calls().is_empty(),
            "no system backend for AppImage"
        );
    }

    #[tokio::test]
    async fn update_of_installed_app_is_allowed() {
        let rig = Rig::new();
        let p = rig.preview(
            PackageFormat::Rpm,
            Compatibility::AlreadyInstalled {
                relation: VersionRelation::Newer,
                installed: Version::new("1.0"),
            },
        );
        assert!(matches!(
            rig.uc().execute(p.id, &rig.observer).await,
            InstallOutcome::Installed(_)
        ));
    }

    #[tokio::test]
    async fn incompatible_preview_is_refused_and_kept() {
        let rig = Rig::new();
        let p = rig.preview(
            PackageFormat::Rpm,
            Compatibility::AlreadyInstalled {
                relation: VersionRelation::Same,
                installed: Version::new("2.0"),
            },
        );
        let out = rig.uc().execute(p.id, &rig.observer).await;
        assert!(matches!(
            out,
            InstallOutcome::Failed(InstallError::Unknown(_))
        ));
        assert!(rig.system.calls().is_empty());
        assert!(rig.registry.get(p.id).is_some());
    }

    #[tokio::test]
    async fn unknown_preview_is_not_found() {
        let rig = Rig::new();
        let out = rig.uc().execute(PreviewId::new(99), &rig.observer).await;
        assert_eq!(out, InstallOutcome::Failed(InstallError::NotFound));
    }

    #[tokio::test]
    async fn dismissed_permission_dialog_is_cancel_not_error() {
        let rig = Rig::new();
        rig.system.fail_install(InstallError::Cancelled);
        let p = rig.preview(PackageFormat::Rpm, Compatibility::Ok);

        let out = rig.uc().execute(p.id, &rig.observer).await;

        assert_eq!(out, InstallOutcome::Cancelled);
        assert!(rig.ledger.apps().is_empty());
        assert!(
            rig.registry.get(p.id).is_some(),
            "kept so the icon can be dropped again"
        );
    }

    #[tokio::test]
    async fn cancel_during_install_stops_it() {
        let rig = Rig::new();
        rig.system.block_until_cancelled();
        let p = rig.preview(PackageFormat::Rpm, Compatibility::Ok);
        let uc = rig.uc();
        let cancel = crate::use_cases::CancelInstall::new(rig.registry.clone());

        let (out, cancelled) = tokio::join!(uc.execute(p.id, &rig.observer), async {
            cancel.execute(p.id)
        });

        assert!(cancelled, "cancel reached the running install");
        assert_eq!(out, InstallOutcome::Cancelled);
        assert!(rig.ledger.apps().is_empty());
    }

    #[tokio::test]
    async fn system_errors_become_failed_outcomes() {
        let rows = [
            InstallError::Offline,
            InstallError::MissingDependencies,
            InstallError::Conflicts,
            InstallError::NotEnoughSpace,
            InstallError::PermissionDenied,
        ];
        for err in rows {
            let rig = Rig::new();
            rig.system.fail_install(err.clone());
            let p = rig.preview(PackageFormat::Rpm, Compatibility::Ok);
            let out = rig.uc().execute(p.id, &rig.observer).await;
            assert_eq!(out, InstallOutcome::Failed(err.clone()), "{err}");
            assert!(rig.ledger.apps().is_empty());
            assert!(rig.registry.get(p.id).is_some(), "retry possible");
        }
    }

    #[tokio::test]
    async fn plan_failure_fails_before_installing() {
        let rig = Rig::new();
        rig.system
            .fail_plan(BackendError::Unavailable("PackageKit".into()));
        let p = rig.preview(PackageFormat::Rpm, Compatibility::Ok);
        let out = rig.uc().execute(p.id, &rig.observer).await;
        assert_eq!(
            out,
            InstallOutcome::Failed(InstallError::BackendUnavailable)
        );
        assert!(!rig.system.calls().iter().any(|c| c.starts_with("install")));
    }

    #[tokio::test]
    async fn desktop_entry_failure_rolls_back_the_copy() {
        let rig = Rig::new();
        rig.desktop.fail_install(InstallError::NotEnoughSpace);
        let p = rig.preview(PackageFormat::AppImage, Compatibility::Ok);

        let out = rig.uc().execute(p.id, &rig.observer).await;

        assert_eq!(out, InstallOutcome::Failed(InstallError::NotEnoughSpace));
        assert_eq!(rig.user.trashed().len(), 1, "copied file trashed again");
        assert!(rig.ledger.apps().is_empty());
    }

    #[tokio::test]
    async fn user_copy_failure_fails() {
        let rig = Rig::new();
        rig.user.fail_install(InstallError::NotEnoughSpace);
        let p = rig.preview(PackageFormat::AppImage, Compatibility::Ok);
        let out = rig.uc().execute(p.id, &rig.observer).await;
        assert_eq!(out, InstallOutcome::Failed(InstallError::NotEnoughSpace));
        assert!(rig.desktop.calls().is_empty());
    }

    #[tokio::test]
    async fn ledger_failure_does_not_fail_the_install() {
        let rig = Rig::new();
        rig.ledger.fail(BackendError::Io("disk".into()));
        let p = rig.preview(PackageFormat::Rpm, Compatibility::Ok);
        assert!(matches!(
            rig.uc().execute(p.id, &rig.observer).await,
            InstallOutcome::Installed(_)
        ));
    }

    #[tokio::test]
    async fn second_install_of_same_preview_is_busy() {
        let rig = Rig::new();
        rig.system.block_until_cancelled();
        let p = rig.preview(PackageFormat::Rpm, Compatibility::Ok);
        let uc = rig.uc();
        let second = rig.uc();

        let (first, busy) = tokio::join!(uc.execute(p.id, &rig.observer), async {
            let out = second.execute(p.id, &RecordingObserver::default()).await;
            rig.registry.cancel(p.id);
            out
        });

        assert_eq!(busy, InstallOutcome::Failed(InstallError::Busy));
        assert_eq!(first, InstallOutcome::Cancelled);
    }
}
