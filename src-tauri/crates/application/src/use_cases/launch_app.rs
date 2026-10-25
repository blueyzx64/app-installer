use std::sync::Arc;

use installer_domain::{AppId, InstallError};

use crate::ports::{AppLedger, Launcher};

/// Start an installed app.
pub struct LaunchApp<L, Lc> {
    ledger: Arc<L>,
    launcher: Arc<Lc>,
}

impl<L, Lc> LaunchApp<L, Lc>
where
    L: AppLedger,
    Lc: Launcher,
{
    pub fn new(ledger: Arc<L>, launcher: Arc<Lc>) -> Self {
        Self { ledger, launcher }
    }

    #[tracing::instrument(skip(self), fields(app = %id))]
    pub async fn execute(&self, id: &AppId) -> Result<(), InstallError> {
        let app = self
            .ledger
            .list()
            .await?
            .into_iter()
            .find(|a| &a.id == id)
            .ok_or(InstallError::NotFound)?;
        if app.desktop_id.is_none() {
            return Err(InstallError::NotFound);
        }
        Ok(self.launcher.launch(&app).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::*;
    use installer_domain::{AppSource, BackendError};

    fn uc(
        ledger: &Arc<FakeLedger>,
        launcher: &Arc<FakeLauncher>,
    ) -> LaunchApp<FakeLedger, FakeLauncher> {
        LaunchApp::new(ledger.clone(), launcher.clone())
    }

    #[tokio::test]
    async fn launches_known_app() {
        let (ledger, launcher) = (
            Arc::new(FakeLedger::default()),
            Arc::new(FakeLauncher::default()),
        );
        ledger.seed(sample_app("lumen", "1", AppSource::AppImage));
        uc(&ledger, &launcher)
            .execute(&AppId::from_name("lumen"))
            .await
            .unwrap();
        assert_eq!(launcher.launched(), ["lumen"]);
    }

    #[tokio::test]
    async fn unknown_app_or_no_desktop_entry_is_not_found() {
        let (ledger, launcher) = (
            Arc::new(FakeLedger::default()),
            Arc::new(FakeLauncher::default()),
        );
        let mut app = sample_app("lumen", "1", AppSource::SystemPackage);
        app.desktop_id = None;
        ledger.seed(app);
        for id in ["lumen", "other"] {
            let err = uc(&ledger, &launcher)
                .execute(&AppId::from_name(id))
                .await
                .unwrap_err();
            assert_eq!(err, InstallError::NotFound, "{id}");
        }
        assert!(launcher.launched().is_empty());
    }

    #[tokio::test]
    async fn launcher_failure_is_mapped() {
        let (ledger, launcher) = (
            Arc::new(FakeLedger::default()),
            Arc::new(FakeLauncher::default()),
        );
        ledger.seed(sample_app("lumen", "1", AppSource::AppImage));
        launcher.fail(BackendError::Failed("exec".into()));
        let err = uc(&ledger, &launcher)
            .execute(&AppId::from_name("lumen"))
            .await
            .unwrap_err();
        assert_eq!(err, InstallError::Unknown("exec".into()));
    }
}
