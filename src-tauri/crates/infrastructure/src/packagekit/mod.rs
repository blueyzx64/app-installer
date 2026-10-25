//! `SystemPackages` over PackageKit (org.freedesktop.PackageKit, system bus).
//!
//! One transaction per operation. polkit asks for permission itself
//! (ADR-0007); we never see a password. A dismissed dialog is a cancel.
//!
//! - `enums`        wire values (⚠ verify against pk-enum.h)
//! - `mapping`      pure fns: status/error/exit/plan mapping
//! - `transaction`  `PkBus`/`PkTransaction` traits and the driver
//! - `proxy`        zbus proxies and the real bus

pub mod enums;
pub mod mapping;
pub mod proxy;
pub mod transaction;

#[cfg(test)]
mod fake;

use installer_application::CancelToken;
use installer_application::ports::{InstallObserver, SystemPackages};
use installer_domain::{BackendError, InstallError, InstallPlan, PackageFile, Version};

use enums::{filter, flag};
use mapping::{PackageId, backend_error, call_error_to_install};
use proxy::SystemBus;
use transaction::{CallError, PkBus, PkOp, TxReport, run};

pub struct PackageKitSystemPackages<B: PkBus = SystemBus> {
    bus: B,
}

impl PackageKitSystemPackages<SystemBus> {
    pub fn new() -> Self {
        Self::with_bus(SystemBus::new())
    }
}

impl Default for PackageKitSystemPackages<SystemBus> {
    fn default() -> Self {
        Self::new()
    }
}

fn install_err(e: CallError) -> InstallError {
    call_error_to_install(e.dbus_name.as_deref(), &e.message)
}

fn path_str(file: &PackageFile) -> Result<String, InstallError> {
    file.path
        .to_str()
        .map(str::to_owned)
        .ok_or(InstallError::NotFound)
}

impl<B: PkBus> PackageKitSystemPackages<B> {
    pub fn with_bus(bus: B) -> Self {
        Self { bus }
    }

    /// Is PackageKit on the system bus (running or activatable)?
    pub async fn is_available(&self) -> bool {
        self.bus.is_available().await
    }

    /// Run a read-only or simulated operation without progress.
    async fn query(&self, op: PkOp) -> Result<TxReport, BackendError> {
        let tx = self.bus.begin().await?;
        let report = run(&tx, op, None, &CancelToken::new())
            .await
            .map_err(|e| backend_error(install_err(e)))?;
        report.result().map_err(backend_error)?;
        Ok(report)
    }

    /// Installed package ids for `name` (any arch/version).
    async fn installed_ids(&self, name: &str) -> Result<Vec<PackageId>, BackendError> {
        let report = self
            .query(PkOp::Resolve {
                filter: filter::INSTALLED,
                names: vec![name.to_owned()],
            })
            .await?;
        Ok(report
            .packages
            .iter()
            .filter_map(|p| PackageId::parse(&p.id))
            .filter(|id| id.name == name)
            .collect())
    }

    /// Trust policy in one place (F-16 will revisit): try trusted first, and
    /// only a missing signature escalates to an untrusted install.
    async fn install_files(
        &self,
        path: String,
        observer: &dyn InstallObserver,
        cancel: &CancelToken,
    ) -> Result<(), InstallError> {
        let mut flags = flag::ONLY_TRUSTED;
        loop {
            let tx = self.bus.begin().await.map_err(InstallError::from)?;
            let report = run(
                &tx,
                PkOp::InstallFiles {
                    flags,
                    paths: vec![path.clone()],
                },
                Some(observer),
                cancel,
            )
            .await
            .map_err(install_err)?;
            let retry = flags == flag::ONLY_TRUSTED
                && mapping::should_retry_untrusted(
                    report.exit,
                    report.error.as_ref().map(|(c, _)| *c),
                );
            if retry {
                flags = flag::NONE;
                continue;
            }
            return report.result();
        }
    }
}

fn package_id_strings(ids: &[PackageId]) -> Vec<String> {
    ids.iter()
        .map(|i| format!("{};{};{};{}", i.name, i.version, i.arch, i.data))
        .collect()
}

impl<B: PkBus> SystemPackages for PackageKitSystemPackages<B> {
    async fn installed_version(&self, name: &str) -> Result<Option<Version>, BackendError> {
        Ok(self
            .installed_ids(name)
            .await?
            .iter()
            .map(PackageId::version)
            .max())
    }

    async fn plan(&self, file: &PackageFile) -> Result<InstallPlan, BackendError> {
        let path = path_str(file).map_err(backend_error)?;
        let report = self
            .query(PkOp::InstallFiles {
                flags: flag::SIMULATE,
                paths: vec![path],
            })
            .await?;
        let extras = mapping::extra_install_ids(&report.packages);
        let mut bytes = 0u64;
        if !extras.is_empty() {
            // Size is best effort: a failure here must not hide the plan.
            match self.query(PkOp::GetDetails { ids: extras }).await {
                Ok(d) => bytes = d.sizes.iter().map(|(_, s)| *s).sum(),
                Err(e) => tracing::warn!("PackageKit GetDetails failed: {e}"),
            }
        }
        Ok(mapping::plan_from_simulation(&report.packages, bytes))
    }

    async fn install(
        &self,
        file: &PackageFile,
        observer: &dyn InstallObserver,
        cancel: CancelToken,
    ) -> Result<(), InstallError> {
        let path = path_str(file)?;
        self.install_files(path, observer, &cancel).await
    }

    /// How many packages besides the app itself an uninstall would remove (F-42).
    async fn removal_plan(&self, name: &str) -> Result<u32, BackendError> {
        let ids = self.installed_ids(name).await?;
        if ids.is_empty() {
            return Ok(0);
        }
        let report = self
            .query(PkOp::RemovePackages {
                flags: flag::SIMULATE,
                ids: package_id_strings(&ids),
                allow_deps: false,
                autoremove: true,
            })
            .await?;
        Ok(mapping::extra_removals(&report.packages))
    }

    async fn remove(&self, name: &str, observer: &dyn InstallObserver) -> Result<(), InstallError> {
        let ids = self.installed_ids(name).await.map_err(InstallError::from)?;
        if ids.is_empty() {
            return Err(InstallError::NotFound);
        }
        let tx = self.bus.begin().await.map_err(InstallError::from)?;
        let report = run(
            &tx,
            PkOp::RemovePackages {
                flags: flag::NONE,
                ids: package_id_strings(&ids),
                allow_deps: false,
                autoremove: true,
            },
            Some(observer),
            &CancelToken::new(),
        )
        .await
        .map_err(install_err)?;
        report.result()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use enums::{PkError, PkExit, PkInfo, PkStatus};
    use fake::{FakeBus, FakeTx, Step};
    use installer_application::testing::RecordingObserver;
    use installer_domain::{InstallPhase, PackageFormat};
    use mapping::PkPackage;
    use transaction::PkEvent;

    fn pkg(info: PkInfo, id: &str) -> Step {
        Step::Event(PkEvent::Package(PkPackage {
            info,
            id: id.into(),
        }))
    }

    fn finished(exit: PkExit) -> Step {
        Step::Event(PkEvent::Finished { exit })
    }

    fn err(code: PkError) -> Step {
        Step::Event(PkEvent::ErrorCode {
            code,
            details: "d".into(),
        })
    }

    fn file() -> PackageFile {
        PackageFile {
            path: "/tmp/a.rpm".into(),
            format: PackageFormat::Rpm,
            size_bytes: 1,
        }
    }

    fn sys(txs: Vec<FakeTx>) -> PackageKitSystemPackages<FakeBus> {
        PackageKitSystemPackages::with_bus(FakeBus::new(txs))
    }

    #[tokio::test]
    async fn installed_version_picks_highest() {
        let tx = FakeTx::new(vec![
            pkg(PkInfo::Installed, "kernel;6.1-1.fc45;x86_64;installed"),
            pkg(PkInfo::Installed, "kernel;6.10-1.fc45;x86_64;installed"),
            pkg(PkInfo::Installed, "kernel-core;9.9;x86_64;installed"),
            finished(PkExit::Success),
        ]);
        let s = sys(vec![tx.clone()]);
        let v = s.installed_version("kernel").await.unwrap();
        assert_eq!(v, Some(Version::new("6.10-1.fc45")));
        assert_eq!(
            tx.ops(),
            vec![PkOp::Resolve {
                filter: filter::INSTALLED,
                names: vec!["kernel".into()]
            }]
        );
    }

    #[tokio::test]
    async fn installed_version_none_and_error() {
        let s = sys(vec![FakeTx::new(vec![finished(PkExit::Success)])]);
        assert_eq!(s.installed_version("x").await, Ok(None));
        let s = sys(vec![FakeTx::new(vec![
            err(PkError::NoNetwork),
            finished(PkExit::Failed),
        ])]);
        assert_eq!(s.installed_version("x").await, Err(BackendError::Offline));
    }

    #[tokio::test]
    async fn plan_counts_extras_and_sums_sizes() {
        let sim = FakeTx::new(vec![
            pkg(PkInfo::Installing, "app;1;x86_64;local"),
            pkg(PkInfo::Installing, "libfoo;2;x86_64;fedora"),
            pkg(PkInfo::Installing, "libbar;3;noarch;fedora"),
            finished(PkExit::Success),
        ]);
        let details = FakeTx::new(vec![
            Step::Event(PkEvent::Details {
                package_id: "libfoo;2;x86_64;fedora".into(),
                size: 1000,
            }),
            Step::Event(PkEvent::Details {
                package_id: "libbar;3;noarch;fedora".into(),
                size: 234,
            }),
            finished(PkExit::Success),
        ]);
        let s = sys(vec![sim.clone(), details.clone()]);
        let plan = s.plan(&file()).await.unwrap();
        assert_eq!(plan.extra_packages, 2);
        assert_eq!(plan.download_bytes, 1234);
        assert_eq!(
            sim.ops(),
            vec![PkOp::InstallFiles {
                flags: flag::SIMULATE,
                paths: vec!["/tmp/a.rpm".into()]
            }]
        );
    }

    #[tokio::test]
    async fn plan_without_extras_skips_details_and_survives_details_failure() {
        let sim = FakeTx::new(vec![
            pkg(PkInfo::Installing, "app;1;x86_64;local"),
            finished(PkExit::Success),
        ]);
        // no second transaction queued: would fail if asked
        let plan = sys(vec![sim]).plan(&file()).await.unwrap();
        assert_eq!(plan, InstallPlan::default());

        let sim = FakeTx::new(vec![
            pkg(PkInfo::Installing, "app;1;x86_64;local"),
            pkg(PkInfo::Installing, "dep;1;x86_64;fedora"),
            finished(PkExit::Success),
        ]);
        let plan = sys(vec![sim]).plan(&file()).await.unwrap();
        assert_eq!(plan.extra_packages, 1);
        assert_eq!(plan.download_bytes, 0);
    }

    #[tokio::test]
    async fn plan_failure_is_backend_error() {
        let sim = FakeTx::new(vec![
            err(PkError::DepResolutionFailed),
            finished(PkExit::Failed),
        ]);
        assert!(matches!(
            sys(vec![sim]).plan(&file()).await,
            Err(BackendError::Failed(_))
        ));
    }

    fn install_script() -> Vec<Step> {
        vec![
            Step::Event(PkEvent::Status(PkStatus::WaitingForAuth)),
            Step::Event(PkEvent::Status(PkStatus::Install)),
            Step::Event(PkEvent::Percentage(50)),
            finished(PkExit::Success),
        ]
    }

    #[tokio::test]
    async fn install_trusted_first_and_reports_phases() {
        let tx = FakeTx::new(install_script());
        let obs = RecordingObserver::default();
        sys(vec![tx.clone()])
            .install(&file(), &obs, CancelToken::new())
            .await
            .unwrap();
        assert_eq!(
            tx.ops(),
            vec![PkOp::InstallFiles {
                flags: flag::ONLY_TRUSTED,
                paths: vec!["/tmp/a.rpm".into()]
            }]
        );
        assert!(obs.phases().contains(&InstallPhase::WaitingForPermission));
        assert_eq!(obs.phases().last(), Some(&InstallPhase::Installing));
    }

    #[tokio::test]
    async fn install_retries_untrusted_on_missing_signature_only() {
        let first = FakeTx::new(vec![
            err(PkError::MissingGpgSignature),
            finished(PkExit::Failed),
        ]);
        let second = FakeTx::new(install_script());
        let s = sys(vec![first.clone(), second.clone()]);
        s.install(&file(), &RecordingObserver::default(), CancelToken::new())
            .await
            .unwrap();
        assert_eq!(
            second.ops(),
            vec![PkOp::InstallFiles {
                flags: flag::NONE,
                paths: vec!["/tmp/a.rpm".into()]
            }]
        );

        // A tampered signature is final.
        let bad = FakeTx::new(vec![
            err(PkError::BadGpgSignature),
            finished(PkExit::Failed),
        ]);
        let r = sys(vec![bad])
            .install(&file(), &RecordingObserver::default(), CancelToken::new())
            .await;
        assert_eq!(r, Err(InstallError::UntrustedSource));

        // Retry happens once: untrusted failing again is final.
        let a = FakeTx::new(vec![
            err(PkError::MissingGpgSignature),
            finished(PkExit::Failed),
        ]);
        let b = FakeTx::new(vec![
            err(PkError::MissingGpgSignature),
            finished(PkExit::Failed),
        ]);
        let r = sys(vec![a, b])
            .install(&file(), &RecordingObserver::default(), CancelToken::new())
            .await;
        assert_eq!(r, Err(InstallError::UntrustedSource));
    }

    #[tokio::test]
    async fn dismissed_polkit_is_cancelled_not_error() {
        // via ErrorCode signal
        let tx = FakeTx::new(vec![
            Step::Event(PkEvent::Status(PkStatus::WaitingForAuth)),
            err(PkError::NotAuthorized),
            finished(PkExit::Failed),
        ]);
        let r = sys(vec![tx])
            .install(&file(), &RecordingObserver::default(), CancelToken::new())
            .await;
        assert_eq!(r, Err(InstallError::Cancelled));

        // via method error
        let tx = FakeTx::new(vec![]).with_call_error(CallError {
            dbus_name: Some("org.freedesktop.PackageKit.Transaction.RefusedByPolicy".into()),
            message: "Failed to obtain authentication.".into(),
        });
        let r = sys(vec![tx])
            .install(&file(), &RecordingObserver::default(), CancelToken::new())
            .await;
        assert_eq!(r, Err(InstallError::Cancelled));
    }

    #[tokio::test]
    async fn install_cancel_calls_cancel_when_allowed() {
        let token = CancelToken::new();
        let tx = FakeTx::new(vec![
            Step::Event(PkEvent::AllowCancel(true)),
            Step::Event(PkEvent::Status(PkStatus::Download)),
            Step::CancelToken(token.clone()),
            Step::Yield,
            Step::Yield,
            finished(PkExit::Cancelled),
        ]);
        let r = sys(vec![tx.clone()])
            .install(&file(), &RecordingObserver::default(), token)
            .await;
        assert_eq!(r, Err(InstallError::Cancelled));
        assert_eq!(tx.cancels(), 1);
    }

    #[tokio::test]
    async fn bus_failure_is_backend_unavailable() {
        let r = sys(vec![])
            .install(&file(), &RecordingObserver::default(), CancelToken::new())
            .await;
        assert_eq!(r, Err(InstallError::BackendUnavailable));
    }

    #[tokio::test]
    async fn remove_resolves_then_removes_with_autoremove() {
        let resolve = FakeTx::new(vec![
            pkg(PkInfo::Installed, "lumen;1.0-1;x86_64;installed"),
            finished(PkExit::Success),
        ]);
        let remove = FakeTx::new(vec![
            Step::Event(PkEvent::Status(PkStatus::Remove)),
            finished(PkExit::Success),
        ]);
        let obs = RecordingObserver::default();
        sys(vec![resolve, remove.clone()])
            .remove("lumen", &obs)
            .await
            .unwrap();
        assert_eq!(
            remove.ops(),
            vec![PkOp::RemovePackages {
                flags: flag::NONE,
                ids: vec!["lumen;1.0-1;x86_64;installed".into()],
                allow_deps: false,
                autoremove: true
            }]
        );
        assert_eq!(obs.phases().last(), Some(&InstallPhase::Installing));
    }

    #[tokio::test]
    async fn remove_not_installed_and_cancelled() {
        let none = FakeTx::new(vec![finished(PkExit::Success)]);
        assert_eq!(
            sys(vec![none])
                .remove("x", &RecordingObserver::default())
                .await,
            Err(InstallError::NotFound)
        );

        let resolve = FakeTx::new(vec![
            pkg(PkInfo::Installed, "x;1;x86_64;installed"),
            finished(PkExit::Success),
        ]);
        let denied = FakeTx::new(vec![err(PkError::NotAuthorized), finished(PkExit::Failed)]);
        assert_eq!(
            sys(vec![resolve, denied])
                .remove("x", &RecordingObserver::default())
                .await,
            Err(InstallError::Cancelled)
        );
    }

    #[tokio::test]
    async fn removal_plan_counts_extra_removals() {
        let resolve = FakeTx::new(vec![
            pkg(PkInfo::Installed, "lumen;1;x86_64;installed"),
            finished(PkExit::Success),
        ]);
        let sim = FakeTx::new(vec![
            pkg(PkInfo::Removing, "lumen;1;x86_64;installed"),
            pkg(PkInfo::Removing, "lumen-libs;1;x86_64;installed"),
            pkg(PkInfo::Removing, "lumen-data;1;noarch;installed"),
            finished(PkExit::Success),
        ]);
        let s = sys(vec![resolve, sim.clone()]);
        assert_eq!(s.removal_plan("lumen").await, Ok(2));
        assert_eq!(
            sim.ops(),
            vec![PkOp::RemovePackages {
                flags: flag::SIMULATE,
                ids: vec!["lumen;1;x86_64;installed".into()],
                allow_deps: false,
                autoremove: true
            }]
        );
        // not installed: nothing to remove
        let none = FakeTx::new(vec![finished(PkExit::Success)]);
        assert_eq!(sys(vec![none]).removal_plan("zzz").await, Ok(0));
    }

    #[tokio::test]
    async fn availability_probe() {
        let mut bus = FakeBus::new(vec![]);
        assert!(
            PackageKitSystemPackages::with_bus(FakeBus::new(vec![]))
                .is_available()
                .await
        );
        bus.available = false;
        assert!(!PackageKitSystemPackages::with_bus(bus).is_available().await);
    }
}
