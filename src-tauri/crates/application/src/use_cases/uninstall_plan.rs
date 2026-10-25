use std::sync::Arc;

use installer_domain::{AppId, AppSource, InstallError, UninstallPlan as Plan};

use crate::ports::{AppLedger, SystemPackages};

/// What removing an app would do, shown before the person confirms (F-42).
///
/// AppImage: nothing extra, no permission. System package: ask the backend how
/// many other packages go along; removal needs permission.
pub struct UninstallPlan<S, L> {
    system: Arc<S>,
    ledger: Arc<L>,
}

impl<S, L> UninstallPlan<S, L>
where
    S: SystemPackages,
    L: AppLedger,
{
    pub fn new(system: Arc<S>, ledger: Arc<L>) -> Self {
        Self { system, ledger }
    }

    #[tracing::instrument(skip(self), fields(app = %id))]
    pub async fn execute(&self, id: &AppId) -> Result<Plan, InstallError> {
        let app = self
            .ledger
            .list()
            .await?
            .into_iter()
            .find(|a| &a.id == id)
            .ok_or(InstallError::NotFound)?;
        match app.source {
            AppSource::AppImage => Ok(Plan {
                also_removes: 0,
                needs_permission: false,
            }),
            AppSource::SystemPackage => Ok(Plan {
                also_removes: self.system.removal_plan(&app.name).await?,
                needs_permission: true,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::*;
    use installer_domain::BackendError;

    fn rig() -> (
        Arc<FakeSystemPackages>,
        Arc<FakeLedger>,
        UninstallPlan<FakeSystemPackages, FakeLedger>,
    ) {
        let s = Arc::new(FakeSystemPackages::default());
        let l = Arc::new(FakeLedger::default());
        let uc = UninstallPlan::new(s.clone(), l.clone());
        (s, l, uc)
    }

    #[tokio::test]
    async fn appimage_needs_nothing() {
        let (s, l, uc) = rig();
        l.seed(sample_app("lumen", "1", AppSource::AppImage));
        let p = uc.execute(&AppId::from_name("lumen")).await.unwrap();
        assert_eq!(p.also_removes, 0);
        assert!(!p.needs_permission);
        assert!(s.calls().is_empty());
    }

    #[tokio::test]
    async fn system_app_reports_extra_removals() {
        let (s, l, uc) = rig();
        l.seed(sample_app("lumen", "1", AppSource::SystemPackage));
        s.set_removal_plan(3);
        let p = uc.execute(&AppId::from_name("lumen")).await.unwrap();
        assert_eq!(p.also_removes, 3);
        assert!(p.needs_permission);
        assert_eq!(s.calls(), ["removal_plan lumen"]);
    }

    #[tokio::test]
    async fn unknown_app_is_not_found() {
        let (_, _, uc) = rig();
        assert_eq!(
            uc.execute(&AppId::from_name("nope")).await,
            Err(InstallError::NotFound)
        );
    }

    #[tokio::test]
    async fn backend_failure_is_an_error() {
        let (s, l, uc) = rig();
        l.seed(sample_app("lumen", "1", AppSource::SystemPackage));
        s.fail_removal_plan(BackendError::Unavailable("pk".into()));
        assert!(uc.execute(&AppId::from_name("lumen")).await.is_err());
    }
}
