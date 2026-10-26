use std::sync::Arc;

use installer_domain::{AppSource, BackendError, InstalledApp};

use crate::identity;
use crate::ports::{AppLedger, DesktopIntegration, SystemPackages, UserApps};

/// The apps we installed, with the ledger reconciled against reality (D-037).
///
/// - AppImage apps are matched to store files by path, never by a recomputed slug. A
///   matched entry keeps its ledger id; its version is refreshed from the file. An entry
///   whose file is gone is forgotten and its menu entry and icons removed (best effort),
///   unless the same app (same identity key) now sits at another path: then the entry
///   follows the file. A file that exists but cannot be read keeps its entry as it is.
///   Readable files that no entry knows are added, with an id from the ledger rules.
/// - If the store cannot be read at all, AppImage entries are kept untouched.
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
        let mut scanned = match self.user.scan().await {
            Ok(apps) => Some(apps),
            Err(e) => {
                tracing::warn!(error = %e, "app store scan failed; keeping ledger entries");
                None
            }
        };

        let mut result: Vec<InstalledApp> = Vec::new();
        let mut gone: Vec<InstalledApp> = Vec::new();
        for mut app in recorded {
            match app.source {
                AppSource::AppImage => match scanned.as_mut() {
                    None => result.push(app),
                    Some(files) => match take_by_path(files, &app) {
                        Some(file) => {
                            self.refresh(&mut app, &file).await;
                            self.repair(&mut app, &file).await;
                            result.push(app);
                        }
                        None => gone.push(app),
                    },
                },
                AppSource::SystemPackage => {
                    let keep = match self.system.installed_version(&app.name).await {
                        Ok(v) => v.is_some(),
                        Err(e) => {
                            tracing::warn!(error = %e, "installed_version failed; keeping entry");
                            true
                        }
                    };
                    if keep {
                        result.push(app);
                    } else if let Err(e) = self.ledger.forget(&app.id).await {
                        tracing::warn!(error = %e, "could not forget stale app");
                    }
                }
            }
        }

        let mut files = scanned.unwrap_or_default();
        for mut app in gone {
            // Same app moved or renamed inside the store: follow the file.
            let moved = files
                .iter()
                .position(|f| !f.name.is_empty() && f.name == app.name);
            if let Some(i) = moved {
                let file = files.swap_remove(i);
                app.path = file.path.clone();
                self.refresh(&mut app, &file).await;
                if let Err(e) = self.ledger.record(&app).await {
                    tracing::warn!(error = %e, "could not record moved app");
                }
                // The menu entry points at the old path: write it again.
                self.rewrite_entry(&mut app, &file).await;
                result.push(app);
                continue;
            }
            if let Err(e) = self.desktop.remove_entry(&app).await {
                tracing::warn!(error = %e, "could not remove stale desktop entry");
            }
            if let Err(e) = self.ledger.forget(&app.id).await {
                tracing::warn!(error = %e, "could not forget stale app");
            }
        }

        // Files no entry knows: add the readable ones.
        for mut file in files.into_iter().filter(|f| !f.name.is_empty()) {
            file.id =
                identity::allocate(&result, AppSource::AppImage, &file.name, &file.display_name);
            if let Err(e) = self.ledger.record(&file).await {
                tracing::warn!(error = %e, "could not record scanned app");
            }
            result.push(file);
        }

        result.sort_by_cached_key(|a| (a.display_name.to_lowercase(), a.id.clone()));
        Ok(result)
    }

    /// The file is there but its menu entry or icon is not (for example the folder was on a
    /// drive that was away): write them again so "Open" works.
    async fn repair(&self, app: &mut InstalledApp, file: &InstalledApp) {
        if file.name.is_empty() || self.desktop.entry_intact(app, file.icon.is_some()).await {
            return;
        }
        self.rewrite_entry(app, file).await;
    }

    async fn rewrite_entry(&self, app: &mut InstalledApp, file: &InstalledApp) {
        match self.desktop.install_entry(app, file.icon.as_ref()).await {
            Ok(desktop_id) => {
                if app.desktop_id.as_deref() != Some(desktop_id.as_str()) {
                    app.desktop_id = Some(desktop_id);
                    if let Err(e) = self.ledger.record(app).await {
                        tracing::warn!(error = %e, "could not record repaired entry");
                    }
                }
            }
            Err(e) => tracing::warn!(error = %e, "could not repair desktop entry"),
        }
    }

    /// The file is the truth for the version (an unreadable file tells us nothing).
    async fn refresh(&self, app: &mut InstalledApp, file: &InstalledApp) {
        if file.version.as_str().is_empty() || app.version == file.version {
            return;
        }
        app.version = file.version.clone();
        if let Err(e) = self.ledger.record(app).await {
            tracing::warn!(error = %e, "could not refresh app version");
        }
    }
}

/// Remove and return the scanned file at the entry's path.
fn take_by_path(files: &mut Vec<InstalledApp>, app: &InstalledApp) -> Option<InstalledApp> {
    let path = app.path.as_ref()?;
    let i = files.iter().position(|f| f.path.as_ref() == Some(path))?;
    Some(files.swap_remove(i))
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

    /// A scanned file (provisional id from its name) at `path`.
    fn file_at(id: &str, key: &str, version: &str, path: &str) -> InstalledApp {
        let mut f = sample_app(id, version, AppSource::AppImage);
        f.name = key.into();
        f.path = Some(path.into());
        f.desktop_id = None;
        f
    }

    #[tokio::test]
    async fn matched_by_path_keeps_ledger_id_and_refreshes_version() {
        let rig = Rig::new();
        let mut known = sample_app("lumen-notes-2", "1.0", AppSource::AppImage);
        known.name = "org.lumen.Notes".into();
        known.path = Some("/s/Lumen.AppImage".into());
        rig.ledger.seed(known);
        // Scan's own id guess differs; the path decides.
        rig.user.set_scan(vec![file_at(
            "lumen-notes",
            "org.lumen.Notes",
            "2.0",
            "/s/Lumen.AppImage",
        )]);
        let apps = rig.uc().execute().await.unwrap();
        assert_eq!(ids(&apps), ["lumen-notes-2"]);
        assert_eq!(apps[0].version.as_str(), "2.0");
        assert_eq!(rig.ledger.apps()[0].version.as_str(), "2.0");
        assert!(rig.desktop.calls().is_empty());
    }

    #[tokio::test]
    async fn unreadable_file_keeps_entry_untouched() {
        let rig = Rig::new();
        let mut known = sample_app("lumen", "1.0", AppSource::AppImage);
        known.path = Some("/s/Lumen.AppImage".into());
        rig.ledger.seed(known.clone());
        rig.user
            .set_scan(vec![file_at("lumen", "", "", "/s/Lumen.AppImage")]);
        let apps = rig.uc().execute().await.unwrap();
        assert_eq!(apps, [known]);
        assert!(rig.desktop.calls().is_empty(), "menu entry kept, no repair");
    }

    #[tokio::test]
    async fn unknown_unreadable_files_are_not_adopted() {
        let rig = Rig::new();
        rig.user
            .set_scan(vec![file_at("junk", "", "", "/s/Junk.AppImage")]);
        assert!(rig.uc().execute().await.unwrap().is_empty());
        assert!(rig.ledger.apps().is_empty());
    }

    #[tokio::test]
    async fn renamed_file_of_same_app_is_followed() {
        let rig = Rig::new();
        let mut known = sample_app("lumen", "1.0", AppSource::AppImage);
        known.name = "org.lumen.Notes".into();
        known.path = Some("/s/Lumen.AppImage".into());
        rig.ledger.seed(known);
        rig.user.set_scan(vec![file_at(
            "x",
            "org.lumen.Notes",
            "1.0",
            "/s/Lumen (old).AppImage",
        )]);
        let apps = rig.uc().execute().await.unwrap();
        assert_eq!(ids(&apps), ["lumen"]);
        assert_eq!(
            rig.ledger.apps()[0].path.as_deref(),
            Some(std::path::Path::new("/s/Lumen (old).AppImage"))
        );
        assert_eq!(
            rig.desktop.calls(),
            ["install lumen"],
            "entry rewritten for new path"
        );
    }

    #[tokio::test]
    async fn adopted_file_gets_a_free_id() {
        let rig = Rig::new();
        rig.ledger
            .seed(sample_app("lumen-notes", "1", AppSource::SystemPackage));
        rig.system.set_installed("lumen-notes", "1");
        let mut f = file_at("whatever", "lumen-notes-abcd", "1", "/s/L.AppImage");
        f.display_name = "Lumen Notes".into();
        rig.user.set_scan(vec![f]);
        let apps = rig.uc().execute().await.unwrap();
        let mut got = ids(&apps);
        got.sort();
        assert_eq!(got, ["lumen-notes", "lumen-notes-2"]);
    }

    #[tokio::test]
    async fn store_unavailable_forgets_and_removes_nothing() {
        let rig = Rig::new();
        rig.ledger.seed(sample_app("img", "1", AppSource::AppImage));
        rig.user.fail_scan(InstallError::NotFound); // root missing / unmounted
        let apps = rig.uc().execute().await.unwrap();
        assert_eq!(ids(&apps), ["img"]);
        assert_eq!(rig.ledger.apps().len(), 1);
        assert!(rig.desktop.calls().is_empty());
    }

    #[tokio::test]
    async fn missing_menu_entry_is_repaired() {
        let rig = Rig::new();
        let known = sample_app("lumen", "1", AppSource::AppImage);
        rig.ledger.seed(known.clone());
        rig.user.set_scan(vec![file_at(
            "lumen",
            "lumen",
            "1",
            known.path.as_deref().unwrap().to_str().unwrap(),
        )]);
        rig.desktop.set_missing(&known.id);
        let apps = rig.uc().execute().await.unwrap();
        assert_eq!(ids(&apps), ["lumen"]);
        assert_eq!(rig.desktop.calls(), ["install lumen"]);
        assert_eq!(
            rig.ledger.apps()[0].desktop_id.as_deref(),
            Some("io.installer.lumen.desktop")
        );
        // Intact now: the next list does nothing.
        rig.uc().execute().await.unwrap();
        assert_eq!(rig.desktop.calls().len(), 1);
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
        // New files get ids from their display names (D-037).
        assert_eq!(ids(&apps), ["apple", "banana", "cherry"]);
    }
}
