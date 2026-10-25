use std::collections::HashSet;
use std::sync::Arc;

use installer_domain::{AppId, AppSource, BackendError, InstalledApp};

use crate::ports::{AppLedger, DesktopIntegration, SystemPackages, UserApps};

/// The apps we installed, with the ledger reconciled against reality.
///
/// - AppImage apps: kept if the store still has them; apps found in the store but
///   missing from the ledger are added to it; ledger entries whose file is gone are forgotten
///   and their desktop entry and icons removed (best effort).
/// - System packages: kept while the package is installed. If the backend cannot
///   answer, the entry is kept (a broken backend must not wipe the library).
pub struct ListInstalledApps<U, L, S, D> {
    user: Arc<U>,
    ledger: Arc<L>,
    system: Arc<S>,
    desktop: Arc<D>,
}

impl<U, L, S, D> ListInstalledApps<U, L, S, D>
where
    U: UserApps,
    L: AppLedger,
    S: SystemPackages,
    D: DesktopIntegration,
{
    pub fn new(user: Arc<U>, ledger: Arc<L>, system: Arc<S>, desktop: Arc<D>) -> Self {
        Self {
            user,
            ledger,
            system,
            desktop,
        }
    }

    /// Sorted by display name, case-insensitive. Fails only if the ledger cannot be read.
    #[tracing::instrument(skip(self))]
    pub async fn execute(&self) -> Result<Vec<InstalledApp>, BackendError> {
        let recorded = self.ledger.list().await?;
        let scanned = match self.user.scan().await {
            Ok(apps) => Some(apps),
            Err(e) => {
                tracing::warn!(error = %e, "app store scan failed; keeping ledger entries");
                None
            }
        };
        let on_disk: HashSet<AppId> = scanned.iter().flatten().map(|a| a.id.clone()).collect();

        let mut result: Vec<InstalledApp> = Vec::new();
        for app in recorded {
            let keep = match app.source {
                AppSource::AppImage => scanned.is_none() || on_disk.contains(&app.id),
                AppSource::SystemPackage => match self.system.installed_version(&app.name).await {
                    Ok(v) => v.is_some(),
                    Err(e) => {
                        tracing::warn!(error = %e, "installed_version failed; keeping entry");
                        true
                    }
                },
            };
            if keep {
                result.push(app);
            } else {
                if app.source == AppSource::AppImage {
                    // File is gone: drop the menu entry and icons too.
                    if let Err(e) = self.desktop.remove_entry(&app).await {
                        tracing::warn!(error = %e, "could not remove stale desktop entry");
                    }
                }
                if let Err(e) = self.ledger.forget(&app.id).await {
                    tracing::warn!(error = %e, "could not forget stale app");
                }
            }
        }

        for app in scanned.into_iter().flatten() {
            if !result.iter().any(|a| a.id == app.id) {
                if let Err(e) = self.ledger.record(&app).await {
                    tracing::warn!(error = %e, "could not record scanned app");
                }
                result.push(app);
            }
        }

        result.sort_by_cached_key(|a| (a.display_name.to_lowercase(), a.id.clone()));
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::*;
    use installer_domain::InstallError;

    struct Rig {
        user: Arc<FakeUserApps>,
        ledger: Arc<FakeLedger>,
        system: Arc<FakeSystemPackages>,
        desktop: Arc<FakeDesktop>,
    }

    impl Rig {
        fn new() -> Self {
            Self {
                user: Arc::new(FakeUserApps::default()),
                ledger: Arc::new(FakeLedger::default()),
                system: Arc::new(FakeSystemPackages::default()),
                desktop: Arc::new(FakeDesktop::default()),
            }
        }
        fn uc(
            &self,
        ) -> ListInstalledApps<FakeUserApps, FakeLedger, FakeSystemPackages, FakeDesktop> {
            ListInstalledApps::new(
                self.user.clone(),
                self.ledger.clone(),
                self.system.clone(),
                self.desktop.clone(),
            )
        }
    }

    fn ids(apps: &[InstalledApp]) -> Vec<&str> {
        apps.iter().map(|a| a.id.as_str()).collect()
    }

    #[tokio::test]
    async fn empty_when_nothing_installed() {
        assert!(Rig::new().uc().execute().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn happy_path_merges_sources_sorted() {
        let rig = Rig::new();
        rig.ledger
            .seed(sample_app("zed", "1.0", AppSource::SystemPackage));
        rig.system.set_installed("zed", "1.0");
        rig.ledger
            .seed(sample_app("beta", "1.0", AppSource::AppImage));
        rig.user
            .set_scan(vec![sample_app("beta", "1.0", AppSource::AppImage)]);

        let apps = rig.uc().execute().await.unwrap();
        assert_eq!(ids(&apps), ["beta", "zed"]);
    }

    #[tokio::test]
    async fn stale_entries_are_forgotten() {
        let rig = Rig::new();
        rig.ledger
            .seed(sample_app("gone-image", "1", AppSource::AppImage));
        rig.ledger
            .seed(sample_app("gone-pkg", "1", AppSource::SystemPackage));
        // system fake knows no packages; scan finds nothing

        let apps = rig.uc().execute().await.unwrap();
        assert!(apps.is_empty());
        assert!(rig.ledger.apps().is_empty(), "ledger cleaned");
        assert_eq!(
            rig.desktop.calls(),
            ["remove gone-image"],
            "menu entry removed for the AppImage only"
        );
    }

    #[tokio::test]
    async fn desktop_cleanup_failure_still_forgets() {
        let rig = Rig::new();
        rig.ledger.seed(sample_app("img", "1", AppSource::AppImage));
        rig.desktop.fail_remove(InstallError::Unknown("x".into()));
        assert!(rig.uc().execute().await.unwrap().is_empty());
        assert!(rig.ledger.apps().is_empty());
    }

    #[tokio::test]
    async fn scanned_but_unrecorded_apps_are_adopted() {
        let rig = Rig::new();
        rig.user
            .set_scan(vec![sample_app("found", "1", AppSource::AppImage)]);

        let apps = rig.uc().execute().await.unwrap();
        assert_eq!(ids(&apps), ["found"]);
        assert_eq!(rig.ledger.apps().len(), 1);
    }

    #[tokio::test]
    async fn backend_errors_keep_entries() {
        let rig = Rig::new();
        rig.ledger
            .seed(sample_app("pkg", "1", AppSource::SystemPackage));
        rig.ledger.seed(sample_app("img", "1", AppSource::AppImage));
        rig.system
            .fail_installed_version(BackendError::Unavailable("pk".into()));
        rig.user.fail_scan(InstallError::BackendUnavailable);

        let apps = rig.uc().execute().await.unwrap();
        assert_eq!(ids(&apps), ["img", "pkg"]);
        assert_eq!(rig.ledger.apps().len(), 2, "nothing forgotten");
        assert!(rig.desktop.calls().is_empty(), "entries kept");
    }

    #[tokio::test]
    async fn unreadable_ledger_is_an_error() {
        let rig = Rig::new();
        rig.ledger.fail(BackendError::Io("corrupt".into()));
        assert_eq!(
            rig.uc().execute().await.unwrap_err(),
            BackendError::Io("corrupt".into())
        );
    }

    #[tokio::test]
    async fn sort_ignores_case() {
        let rig = Rig::new();
        let mut a = sample_app("a", "1", AppSource::AppImage);
        a.display_name = "apple".into();
        let mut b = sample_app("b", "1", AppSource::AppImage);
        b.display_name = "Banana".into();
        let mut c = sample_app("c", "1", AppSource::AppImage);
        c.display_name = "cherry".into();
        rig.user.set_scan(vec![c, b, a]);
        let apps = rig.uc().execute().await.unwrap();
        assert_eq!(ids(&apps), ["a", "b", "c"]);
    }
}
