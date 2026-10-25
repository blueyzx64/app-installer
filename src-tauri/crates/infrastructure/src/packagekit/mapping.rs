//! Pure mapping from PackageKit values to domain values. No IO.

use installer_domain::{BackendError, InstallError, InstallPhase, InstallPlan, Version};

use super::enums::{PkError, PkExit, PkInfo, PkStatus};

/// Status to phase (D-019). Everything not listed is `Preparing`.
pub fn status_to_phase(status: PkStatus) -> InstallPhase {
    use PkStatus::*;
    match status {
        WaitingForAuth => InstallPhase::WaitingForPermission,
        Download | DownloadRepository | DownloadPackagelist | DownloadFilelist
        | DownloadChangelog | DownloadGroup | DownloadUpdateinfo => InstallPhase::Downloading,
        Install | Update | Remove | Run | Commit | TestCommit | CopyFiles | RunHook => {
            InstallPhase::Installing
        }
        Cleanup | Finished => InstallPhase::Finishing,
        _ => InstallPhase::Preparing,
    }
}

fn rank(p: InstallPhase) -> u8 {
    match p {
        InstallPhase::Preparing => 0,
        InstallPhase::WaitingForPermission => 1,
        InstallPhase::Downloading => 2,
        InstallPhase::Installing => 3,
        InstallPhase::Finishing => 4,
    }
}

/// D-019: phases only move forward. The one exception is leaving
/// `WaitingForPermission`, which is a pause and may be followed by anything.
pub fn next_phase(current: InstallPhase, incoming: InstallPhase) -> InstallPhase {
    if current == InstallPhase::WaitingForPermission || rank(incoming) >= rank(current) {
        incoming
    } else {
        current
    }
}

/// Raw `Percentage` to a percent. 101 (and anything above 100) is unknown.
pub fn percent(raw: u32) -> Option<u8> {
    u8::try_from(raw).ok().filter(|p| *p <= 100)
}

/// `RemainingTime` seconds; 0 means unknown.
pub fn eta(raw: u32) -> Option<u32> {
    (raw > 0).then_some(raw)
}

/// PackageKit error to install error. `NotAuthorized` is a dismissed polkit
/// dialog and must read as a cancel (F-24).
pub fn error_to_install(code: PkError, details: &str) -> InstallError {
    use PkError::*;
    match code {
        NotAuthorized | TransactionCancelled | CancelledPriority => InstallError::Cancelled,
        NoNetwork
        | RepoNotAvailable
        | NoMoreMirrorsToTry
        | PackageDownloadFailed
        | CannotFetchSources => InstallError::Offline,
        DepResolutionFailed => InstallError::MissingDependencies,
        FileConflicts | PackageConflicts => InstallError::Conflicts,
        NoSpaceOnDevice => InstallError::NotEnoughSpace,
        InvalidPackageFile | PackageCorrupt | IncompatibleArchitecture => InstallError::CorruptFile,
        MissingGpgSignature
        | BadGpgSignature
        | GpgFailure
        | CannotInstallRepoUnsigned
        | CannotUpdateRepoUnsigned => InstallError::UntrustedSource,
        CannotGetLock | UnfinishedTransaction => InstallError::Busy,
        FileNotFound | PackageNotFound | PackageNotInstalled => InstallError::NotFound,
        CannotRemoveSystemPackage | PackageInstallBlocked => InstallError::PermissionDenied,
        other => InstallError::Unknown(format!("{other:?}: {details}")),
    }
}

/// `Finished` exit code with no `ErrorCode` before it.
pub fn exit_to_result(exit: PkExit) -> Result<(), InstallError> {
    match exit {
        PkExit::Success => Ok(()),
        PkExit::Cancelled | PkExit::CancelledPriority | PkExit::Killed => {
            Err(InstallError::Cancelled)
        }
        PkExit::KeyRequired | PkExit::NeedUntrusted | PkExit::EulaRequired => {
            Err(InstallError::UntrustedSource)
        }
        other => Err(InstallError::Unknown(format!("exit {other:?}"))),
    }
}

/// Final result of a transaction: success wins, then the error signal, then the exit code.
pub fn outcome(exit: PkExit, error: Option<(PkError, &str)>) -> Result<(), InstallError> {
    if exit == PkExit::Success {
        return Ok(());
    }
    match error {
        Some((code, details)) => Err(error_to_install(code, details)),
        None => exit_to_result(exit),
    }
}

/// Same, for read-only calls that return `BackendError`.
pub fn backend_error(e: InstallError) -> BackendError {
    match e {
        InstallError::Cancelled | InstallError::PermissionDenied => BackendError::NotAuthorized,
        InstallError::Offline => BackendError::Offline,
        InstallError::BackendUnavailable => BackendError::Unavailable("PackageKit".into()),
        InstallError::Unknown(d) => BackendError::Failed(d),
        other => BackendError::Failed(other.to_string()),
    }
}

/// Should the install be retried without `ONLY_TRUSTED` (the escalation to the
/// `package-install-untrusted` polkit action)? One place for the trust policy;
/// F-16 may change it. `NotAuthorized` is deliberately NOT here: it also means
/// a dismissed dialog. A bad (tampered) signature is never retried.
pub fn should_retry_untrusted(exit: PkExit, error: Option<PkError>) -> bool {
    exit == PkExit::NeedUntrusted
        || matches!(
            error,
            Some(PkError::MissingGpgSignature | PkError::CannotInstallRepoUnsigned)
        )
}

/// D-Bus error name from a failed method call to an install error.
/// `RefusedByPolicy` is polkit saying no or the dialog being dismissed (F-24).
pub fn call_error_to_install(dbus_name: Option<&str>, message: &str) -> InstallError {
    let name = dbus_name.unwrap_or("");
    if name.ends_with("RefusedByPolicy") || name.ends_with("NotAuthorized") {
        InstallError::Cancelled
    } else if name.ends_with("ServiceUnknown")
        || name.ends_with("NameHasNoOwner")
        || name.ends_with("NoReply")
        || name.ends_with("Disconnected")
    {
        InstallError::BackendUnavailable
    } else {
        InstallError::Unknown(format!("{name}: {message}"))
    }
}

/// `name;version;arch;data`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageId {
    pub name: String,
    pub version: String,
    pub arch: String,
    pub data: String,
}

impl PackageId {
    pub fn parse(id: &str) -> Option<PackageId> {
        let mut it = id.split(';');
        let name = it.next().filter(|s| !s.is_empty())?;
        let version = it.next()?;
        let arch = it.next()?;
        let data = it.next()?;
        if it.next().is_some() {
            return None;
        }
        Some(PackageId {
            name: name.into(),
            version: version.into(),
            arch: arch.into(),
            data: data.into(),
        })
    }

    pub fn version(&self) -> Version {
        Version::new(self.version.clone())
    }

    /// A package id of a local file carries `local` (or `local:...`) as data. ⚠ verify on Fedora.
    pub fn is_local(&self) -> bool {
        self.data == "local" || self.data.starts_with("local:")
    }
}

/// One `Package` signal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PkPackage {
    pub info: PkInfo,
    pub id: String,
}

fn adds(info: PkInfo) -> bool {
    matches!(
        info,
        PkInfo::Installing | PkInfo::Updating | PkInfo::Reinstalling | PkInfo::Downgrading
    )
}

/// Package ids a simulated install would add besides the file's own package.
/// The file's own package is the one marked `local`; without a marker, the first.
pub fn extra_install_ids(pkgs: &[PkPackage]) -> Vec<String> {
    let adding: Vec<(&PkPackage, Option<PackageId>)> = pkgs
        .iter()
        .filter(|p| adds(p.info))
        .map(|p| (p, PackageId::parse(&p.id)))
        .collect();
    let has_local = adding
        .iter()
        .any(|(_, id)| id.as_ref().is_some_and(PackageId::is_local));
    let mut out: Vec<String> = Vec::new();
    for (i, (p, id)) in adding.iter().enumerate() {
        let is_target = if has_local {
            id.as_ref().is_some_and(PackageId::is_local)
        } else {
            i == 0
        };
        if !is_target {
            out.push(p.id.clone());
        }
    }
    out
}

/// Plan from a simulated install. `download_bytes` comes from `GetDetails` of the extras.
pub fn plan_from_simulation(pkgs: &[PkPackage], download_bytes: u64) -> InstallPlan {
    InstallPlan {
        extra_packages: u32::try_from(extra_install_ids(pkgs).len()).unwrap_or(u32::MAX),
        download_bytes,
    }
}

/// Extra removals of a simulated remove: every `Removing` package except the app itself.
pub fn extra_removals(pkgs: &[PkPackage]) -> u32 {
    let n = pkgs.iter().filter(|p| p.info == PkInfo::Removing).count();
    u32::try_from(n.saturating_sub(1)).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_table() {
        use InstallPhase::*;
        use PkStatus as S;
        let rows = [
            (S::WaitingForAuth, WaitingForPermission),
            (S::Download, Downloading),
            (S::DownloadPackagelist, Downloading),
            (S::Install, Installing),
            (S::Commit, Installing),
            (S::Run, Installing),
            (S::Remove, Installing),
            (S::Cleanup, Finishing),
            (S::Finished, Finishing),
            (S::Setup, Preparing),
            (S::DepResolve, Preparing),
            (S::WaitingForLock, Preparing),
            (S::Wait, Preparing),
            (S::Unknown, Preparing),
            (S::SigCheck, Preparing),
        ];
        for (s, want) in rows {
            assert_eq!(status_to_phase(s), want, "{s:?}");
        }
    }

    #[test]
    fn phases_never_go_backwards_except_after_waiting() {
        use InstallPhase::*;
        assert_eq!(next_phase(Installing, Preparing), Installing);
        assert_eq!(next_phase(Installing, WaitingForPermission), Installing);
        assert_eq!(
            next_phase(Preparing, WaitingForPermission),
            WaitingForPermission
        );
        assert_eq!(next_phase(WaitingForPermission, Preparing), Preparing);
        assert_eq!(next_phase(WaitingForPermission, Downloading), Downloading);
        assert_eq!(next_phase(Downloading, Installing), Installing);
        assert_eq!(next_phase(Finishing, Installing), Finishing);
    }

    #[test]
    fn percent_and_eta() {
        assert_eq!(percent(0), Some(0));
        assert_eq!(percent(100), Some(100));
        assert_eq!(percent(101), None);
        assert_eq!(percent(u32::MAX), None);
        assert_eq!(eta(0), None);
        assert_eq!(eta(12), Some(12));
    }

    #[test]
    fn error_table() {
        use InstallError as E;
        use PkError as P;
        let rows = [
            (P::NotAuthorized, E::Cancelled),
            (P::TransactionCancelled, E::Cancelled),
            (P::NoNetwork, E::Offline),
            (P::PackageDownloadFailed, E::Offline),
            (P::RepoNotAvailable, E::Offline),
            (P::DepResolutionFailed, E::MissingDependencies),
            (P::FileConflicts, E::Conflicts),
            (P::PackageConflicts, E::Conflicts),
            (P::NoSpaceOnDevice, E::NotEnoughSpace),
            (P::InvalidPackageFile, E::CorruptFile),
            (P::PackageCorrupt, E::CorruptFile),
            (P::MissingGpgSignature, E::UntrustedSource),
            (P::BadGpgSignature, E::UntrustedSource),
            (P::CannotGetLock, E::Busy),
            (P::FileNotFound, E::NotFound),
            (P::CannotRemoveSystemPackage, E::PermissionDenied),
        ];
        for (code, want) in rows {
            assert_eq!(error_to_install(code, "d"), want, "{code:?}");
        }
        assert_eq!(
            error_to_install(P::Oom, "mem"),
            E::Unknown("Oom: mem".into())
        );
    }

    #[test]
    fn exit_and_outcome_table() {
        assert_eq!(outcome(PkExit::Success, None), Ok(()));
        assert_eq!(outcome(PkExit::Success, Some((PkError::Oom, "x"))), Ok(()));
        assert_eq!(
            outcome(PkExit::Cancelled, None),
            Err(InstallError::Cancelled)
        );
        assert_eq!(
            outcome(PkExit::Failed, Some((PkError::NotAuthorized, ""))),
            Err(InstallError::Cancelled)
        );
        assert_eq!(
            outcome(PkExit::Failed, Some((PkError::NoNetwork, ""))),
            Err(InstallError::Offline)
        );
        assert_eq!(
            outcome(PkExit::NeedUntrusted, None),
            Err(InstallError::UntrustedSource)
        );
        assert_eq!(
            outcome(PkExit::Failed, None),
            Err(InstallError::Unknown("exit Failed".into()))
        );
    }

    #[test]
    fn backend_error_table() {
        assert_eq!(
            backend_error(InstallError::Cancelled),
            BackendError::NotAuthorized
        );
        assert_eq!(backend_error(InstallError::Offline), BackendError::Offline);
        assert_eq!(
            backend_error(InstallError::Unknown("x".into())),
            BackendError::Failed("x".into())
        );
    }

    #[test]
    fn retry_policy() {
        assert!(should_retry_untrusted(
            PkExit::Failed,
            Some(PkError::MissingGpgSignature)
        ));
        assert!(should_retry_untrusted(PkExit::NeedUntrusted, None));
        assert!(!should_retry_untrusted(
            PkExit::Failed,
            Some(PkError::NotAuthorized)
        ));
        assert!(!should_retry_untrusted(
            PkExit::Failed,
            Some(PkError::BadGpgSignature)
        ));
        assert!(!should_retry_untrusted(PkExit::Success, None));
    }

    #[test]
    fn call_error_table() {
        let base = "org.freedesktop.PackageKit.Transaction";
        assert_eq!(
            call_error_to_install(Some(&format!("{base}.RefusedByPolicy")), "x"),
            InstallError::Cancelled
        );
        assert_eq!(
            call_error_to_install(Some("org.freedesktop.DBus.Error.ServiceUnknown"), "x"),
            InstallError::BackendUnavailable
        );
        assert!(matches!(
            call_error_to_install(None, "boom"),
            InstallError::Unknown(_)
        ));
    }

    #[test]
    fn package_id_parse() {
        let id = PackageId::parse("lumen;1:1.2-3.fc45;x86_64;local").unwrap();
        assert_eq!(id.name, "lumen");
        assert_eq!(id.version().as_str(), "1:1.2-3.fc45");
        assert_eq!(id.arch, "x86_64");
        assert!(id.is_local());
        assert!(!PackageId::parse("a;1;x86_64;fedora").unwrap().is_local());
        assert!(PackageId::parse("a;1;x86_64").is_none());
        assert!(PackageId::parse(";1;x;y").is_none());
        assert!(PackageId::parse("a;1;x;y;z").is_none());
        assert!(PackageId::parse("").is_none());
    }

    fn p(info: PkInfo, id: &str) -> PkPackage {
        PkPackage {
            info,
            id: id.into(),
        }
    }

    #[test]
    fn plan_counts_extras_with_local_marker() {
        let pkgs = [
            p(PkInfo::Installing, "dep1;1;x86_64;fedora"),
            p(PkInfo::Installing, "app;1;x86_64;local"),
            p(PkInfo::Updating, "dep2;2;noarch;updates"),
            p(PkInfo::Removing, "old;1;x86_64;installed"),
        ];
        let plan = plan_from_simulation(&pkgs, 4096);
        assert_eq!(plan.extra_packages, 2);
        assert_eq!(plan.download_bytes, 4096);
        assert_eq!(
            extra_install_ids(&pkgs),
            vec!["dep1;1;x86_64;fedora", "dep2;2;noarch;updates"]
        );
    }

    #[test]
    fn plan_without_marker_skips_first_and_handles_empty() {
        let pkgs = [
            p(PkInfo::Installing, "app;1;x86_64;x"),
            p(PkInfo::Installing, "dep;1;x86_64;fedora"),
        ];
        assert_eq!(plan_from_simulation(&pkgs, 0).extra_packages, 1);
        assert_eq!(plan_from_simulation(&[], 0), InstallPlan::default());
        assert_eq!(
            plan_from_simulation(&[p(PkInfo::Installing, "app;1;x;local")], 0).extra_packages,
            0
        );
    }

    #[test]
    fn removal_extras() {
        assert_eq!(extra_removals(&[]), 0);
        assert_eq!(extra_removals(&[p(PkInfo::Removing, "a;1;x;i")]), 0);
        assert_eq!(
            extra_removals(&[
                p(PkInfo::Removing, "a;1;x;i"),
                p(PkInfo::Removing, "b;1;x;i"),
                p(PkInfo::Removing, "c;1;x;i"),
                p(PkInfo::Installed, "d;1;x;i"),
            ]),
            2
        );
    }
}
