//! R4 adversarial review proofs. Each test is expected to FAIL on 085485c.
use super::PackageKitSystemPackages;
use super::enums::{PkError, PkExit, PkInfo, PkStatus, flag};
use super::fake::{FakeBus, FakeTx, Step};
use super::mapping::{self, PkPackage};
use super::transaction::{PkEvent, PkOp, run};
use installer_application::CancelToken;
use installer_application::ports::SystemPackages;
use installer_application::testing::RecordingObserver;
use installer_domain::{InstallError, InstallPhase, PackageFile, PackageFormat};

fn p(info: PkInfo, id: &str) -> PkPackage {
    PkPackage {
        info,
        id: id.into(),
    }
}

fn rpm() -> PackageFile {
    PackageFile {
        path: "/tmp/a.rpm".into(),
        format: PackageFormat::Rpm,
        size_bytes: 1,
    }
}

/// PackageKit 1.4.0 src/pk-backend-job.c:974-983 inserts Status RUNNING (3)
/// before the first non-SETUP status. Real order after auth:
/// SETUP, RUNNING, QUERY, DOWNLOAD, ... RUNNING must not mean "Installing".
#[tokio::test]
async fn review_r4_injected_running_hides_downloading() {
    let tx = FakeTx::new(vec![
        Step::Event(PkEvent::Status(PkStatus::WaitingForAuth)),
        Step::Event(PkEvent::Status(PkStatus::Setup)),
        Step::Event(PkEvent::Status(PkStatus::Run)),
        Step::Event(PkEvent::Status(PkStatus::Query)),
        Step::Event(PkEvent::Status(PkStatus::Download)),
        Step::Event(PkEvent::Percentage(30)),
        Step::Event(PkEvent::Status(PkStatus::Install)),
        Step::Event(PkEvent::Finished {
            exit: PkExit::Success,
        }),
    ]);
    let obs = RecordingObserver::default();
    let op = PkOp::InstallFiles {
        flags: 0,
        paths: vec!["/a.rpm".into()],
    };
    run(&tx, op, Some(&obs), &CancelToken::new()).await.unwrap();
    assert!(
        obs.phases().contains(&InstallPhase::Downloading),
        "Downloading never shown: {:?}",
        obs.phases()
    );
}

/// dnf5 backend labels a local file `@commandline` (backends/dnf5/dnf5-backend-utils.hpp:37,
/// id built at dnf5-backend-utils.cpp:398), not `local`.
#[test]
fn review_r4_commandline_repo_is_the_target_not_an_extra() {
    let pkgs = [
        p(PkInfo::Installing, "libdep;1;x86_64;fedora"),
        p(PkInfo::Installing, "app;1;x86_64;@commandline"),
    ];
    assert_eq!(
        mapping::extra_install_ids(&pkgs),
        vec!["libdep;1;x86_64;fedora"]
    );
}

/// Exit KILLED = "Backend was killed" (pk-enum.h), not a person pressing Cancel.
#[test]
fn review_r4_killed_is_not_silent_cancel() {
    assert_ne!(
        mapping::outcome(PkExit::Killed, None),
        Err(InstallError::Cancelled)
    );
}

/// v1.4.0 pk-enum.h: INSTALL=27 REMOVE=28 OBSOLETE=29 DOWNGRADE=30
/// (simulate infos for non-dnf5 backends).
#[test]
fn review_r4_info_enum_covers_1_4_0() {
    assert_ne!(PkInfo::from_u32(27), PkInfo::Unknown);
}

/// Cancel fired while the trusted attempt could not be cancelled; it fails
/// with MissingGpgSignature; the adapter must not start (and prompt for) a
/// second, untrusted transaction.
#[tokio::test]
async fn review_r4_cancel_before_retry_does_not_start_second_transaction() {
    let token = CancelToken::new();
    let first = FakeTx::new(vec![
        Step::Event(PkEvent::Status(PkStatus::WaitingForAuth)),
        Step::CancelToken(token.clone()),
        Step::Yield,
        Step::Event(PkEvent::ErrorCode {
            code: PkError::MissingGpgSignature,
            details: "".into(),
        }),
        Step::Event(PkEvent::Finished {
            exit: PkExit::Failed,
        }),
    ]);
    let second = FakeTx::new(vec![
        Step::Event(PkEvent::Status(PkStatus::WaitingForAuth)),
        Step::Event(PkEvent::Finished {
            exit: PkExit::Success,
        }),
    ]);
    let s = PackageKitSystemPackages::with_bus(FakeBus::new(vec![first, second.clone()]));
    let r = s
        .install(&rpm(), &RecordingObserver::default(), token)
        .await;
    assert!(
        second.ops().is_empty(),
        "second (untrusted) transaction was started after cancel"
    );
    assert_eq!(r, Err(InstallError::Cancelled));
}

/// pk-transaction.c:2837-2843: without ONLY_TRUSTED the untrusted polkit action is asked
/// (second password prompt after the trusted one succeeded). dnf5 backend comment
/// (dnf5-backend-thread.cpp:595-598) says clients SIMULATE|ONLY_TRUSTED and read the
/// UNTRUSTED info to skip the trusted attempt. plan() uses SIMULATE only.
#[tokio::test]
async fn review_r4_plan_simulates_with_only_trusted_to_detect_untrusted() {
    let sim = FakeTx::new(vec![Step::Event(PkEvent::Finished {
        exit: PkExit::Success,
    })]);
    let s = PackageKitSystemPackages::with_bus(FakeBus::new(vec![sim.clone()]));
    let _ = s.plan(&rpm()).await;
    match &sim.ops()[0] {
        PkOp::InstallFiles { flags, .. } => {
            assert_eq!(*flags, flag::SIMULATE | flag::ONLY_TRUSTED)
        }
        o => panic!("{o:?}"),
    }
}

// ---- Re-verification round (b0ba45c): attacks on the fixes ----

/// Real dnf5 order on PackageKit 1.4.0 for InstallFiles/RemovePackages:
/// backends/dnf5/dnf5-backend-thread.cpp:571 QUERY, :672 DOWNLOAD, :713 RUNNING then
/// trans.run(). dnf5 never sets INSTALL/COMMIT; the install itself runs under RUNNING.
/// The fix maps Run -> Preparing, so after Download the phase is stuck on Downloading.
fn dnf5_install_script(final_info: PkInfo) -> Vec<Step> {
    vec![
        Step::Event(PkEvent::Status(PkStatus::WaitingForAuth)),
        Step::Event(PkEvent::Status(PkStatus::Setup)),
        Step::Event(PkEvent::Status(PkStatus::Run)), // injected, pk-backend-job.c:974-983
        Step::Event(PkEvent::Status(PkStatus::Query)),
        Step::Event(PkEvent::Status(PkStatus::Download)),
        Step::Event(PkEvent::Percentage(100)),
        Step::Event(PkEvent::Status(PkStatus::Run)), // :713, the real install
        Step::Event(PkEvent::Package(PkPackage {
            info: final_info,
            id: "app;1;x86_64;@commandline".into(),
        })),
        Step::Event(PkEvent::Percentage(40)),
        Step::Event(PkEvent::Finished {
            exit: PkExit::Success,
        }),
    ]
}

#[tokio::test]
async fn review_r4_rv_dnf5_install_reaches_installing() {
    let tx = FakeTx::new(dnf5_install_script(PkInfo::Installing));
    let obs = RecordingObserver::default();
    let op = PkOp::InstallFiles {
        flags: 0,
        paths: vec!["/a.rpm".into()],
    };
    run(&tx, op, Some(&obs), &CancelToken::new()).await.unwrap();
    let ph = obs.phases();
    let d = ph.iter().position(|p| *p == InstallPhase::Downloading);
    let i = ph.iter().position(|p| *p == InstallPhase::Installing);
    assert!(
        d.is_some() && i.is_some() && d < i,
        "want Downloading then Installing, got {ph:?}"
    );
}

#[tokio::test]
async fn review_r4_rv_dnf5_remove_is_not_downloading() {
    let resolve = FakeTx::new(vec![
        Step::Event(PkEvent::Package(p(
            PkInfo::Installed,
            "app;1;x86_64;installed",
        ))),
        Step::Event(PkEvent::Finished {
            exit: PkExit::Success,
        }),
    ]);
    let remove = FakeTx::new(dnf5_install_script(PkInfo::Removing));
    let obs = RecordingObserver::default();
    PackageKitSystemPackages::with_bus(FakeBus::new(vec![resolve, remove]))
        .remove("app", &obs)
        .await
        .unwrap();
    assert_eq!(
        obs.phases().last(),
        Some(&InstallPhase::Installing),
        "remove ends on {:?}",
        obs.phases()
    );
}

/// Dismissed polkit: daemon emits Status FINISHED, ErrorCode NOT_AUTHORIZED,
/// Finished(FAILED) (pk-transaction.c:2644-2651). The UI is told "Finishing"
/// for an install that never started (D-019, F-24 spring-back).
#[tokio::test]
async fn review_r4_rv_dismissed_polkit_never_shows_finishing() {
    let tx = FakeTx::new(vec![
        Step::Event(PkEvent::Status(PkStatus::WaitingForAuth)),
        Step::Event(PkEvent::Status(PkStatus::Finished)),
        Step::Event(PkEvent::ErrorCode {
            code: PkError::NotAuthorized,
            details: "Failed to obtain authentication.".into(),
        }),
        Step::Event(PkEvent::Finished {
            exit: PkExit::Failed,
        }),
    ]);
    let obs = RecordingObserver::default();
    let r = PackageKitSystemPackages::with_bus(FakeBus::new(vec![tx]))
        .install(&rpm(), &obs, CancelToken::new())
        .await;
    assert_eq!(r, Err(InstallError::Cancelled));
    assert!(
        !obs.phases().contains(&InstallPhase::Finishing),
        "{:?}",
        obs.phases()
    );
}

/// Fix check (passes = OK): untrusted plan (dnf5 emits UNTRUSTED then the
/// normal INSTALLING rows, thread.cpp:607-661) gives one NONE install, and the
/// UNTRUSTED row is not counted as an extra.
#[tokio::test]
async fn review_r4_rv_untrusted_info_plan_single_prompt() {
    let sim = FakeTx::new(vec![
        Step::Event(PkEvent::Package(p(
            PkInfo::Untrusted,
            "app;1;x86_64;@commandline",
        ))),
        Step::Event(PkEvent::Package(p(
            PkInfo::Installing,
            "libdep;1;x86_64;fedora",
        ))),
        Step::Event(PkEvent::Package(p(
            PkInfo::Installing,
            "app;1;x86_64;@commandline",
        ))),
        Step::Event(PkEvent::Finished {
            exit: PkExit::Success,
        }),
    ]);
    let details = FakeTx::new(vec![
        Step::Event(PkEvent::Details {
            package_id: "libdep;1;x86_64;fedora".into(),
            size: 7,
        }),
        Step::Event(PkEvent::Finished {
            exit: PkExit::Success,
        }),
    ]);
    let install = FakeTx::new(vec![Step::Event(PkEvent::Finished {
        exit: PkExit::Success,
    })]);
    let s = PackageKitSystemPackages::with_bus(FakeBus::new(vec![sim, details, install.clone()]));
    let plan = s.plan(&rpm()).await.unwrap();
    assert_eq!((plan.extra_packages, plan.download_bytes), (1, 7));
    s.install(&rpm(), &RecordingObserver::default(), CancelToken::new())
        .await
        .unwrap();
    assert_eq!(
        install.ops(),
        vec![PkOp::InstallFiles {
            flags: flag::NONE,
            paths: vec!["/tmp/a.rpm".into()]
        }]
    );
}
