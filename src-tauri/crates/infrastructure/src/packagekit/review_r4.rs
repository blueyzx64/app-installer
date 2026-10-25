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
