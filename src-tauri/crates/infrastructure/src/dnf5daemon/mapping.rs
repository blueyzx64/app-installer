//! Pure functions: bus data -> domain. No IO, all unit tested.

use installer_domain::{
    BackendError, InstallError, InstallPhase, InstallPlan, InstallProgress, Version,
};

use super::bus::{BusError, Dnf5Signal, InstalledRpm, Resolution};

/// Turn a D-Bus error name (+ message) into a `BusError`. ⚠ exact polkit error
/// names on dnf5daemon unverified (HUMAN: check on F45 with `busctl monitor`).
pub fn classify_dbus_error(name: &str, message: &str) -> BusError {
    let n = name.to_ascii_lowercase();
    let m = message.to_ascii_lowercase();
    // Only polkit's own errors, or dnf5daemon's exact `runtime_error("Not authorized")`
    // (arrives as DBus.Error.Failed + that text), count as a dismissed dialog.
    // Never match loose words: an HTTP 401 "authentication required" is a real failure.
    if n.starts_with("org.freedesktop.policykit1.error.")
        || message.trim().eq_ignore_ascii_case("not authorized")
    {
        BusError::AccessDenied
    } else if n.ends_with("serviceunknown")
        || n.ends_with("namehasnoowner")
        || n.contains("dbus.error.spawn")
        || n.contains("spawn")
        || n.ends_with("noreply")
        || n.ends_with("disconnected")
        || (m.starts_with("process ") && m.contains(" exited"))
    {
        // Activation of org.rpm.dnf.v0 failed ("Process org.rpm.dnf.v0 exited with status 1").
        BusError::Unavailable(name.to_owned())
    } else if m.contains("cancel") {
        BusError::Cancelled
    } else {
        BusError::Failed(if message.is_empty() { name } else { message }.to_owned())
    }
}

fn failure_text(msg: &str) -> InstallError {
    let m = msg.to_ascii_lowercase();
    // dnf words a missing dependency as "conflicting requests - nothing provides ...".
    if m.contains("nothing provides") {
        InstallError::MissingDependencies
    } else if m.contains("conflict") {
        InstallError::Conflicts
    } else if m.contains("no space") || m.contains("disk space") || m.contains("enough space") {
        InstallError::NotEnoughSpace
    } else if m.contains("resolve host")
        || m.contains("failed to download")
        || m.contains("cannot download")
        || m.contains("network")
        || m.contains("offline")
    {
        InstallError::Offline
    } else if m.contains("gpg") || m.contains("signature") {
        InstallError::UntrustedSource
    } else if m.contains("nothing provides")
        || m.contains("requires")
        || m.contains("depend")
        || m.contains("problem")
    {
        InstallError::MissingDependencies
    } else {
        InstallError::Unknown(msg.to_owned())
    }
}

pub fn bus_to_install(e: BusError) -> InstallError {
    match e {
        // A dismissed polkit dialog is a cancel, not an error (ADR-0007).
        BusError::AccessDenied | BusError::Cancelled => InstallError::Cancelled,
        BusError::Unavailable(_) => InstallError::BackendUnavailable,
        BusError::Failed(m) => failure_text(&m),
    }
}

pub fn bus_to_backend(e: BusError) -> BackendError {
    match e {
        BusError::AccessDenied => BackendError::NotAuthorized,
        BusError::Unavailable(m) => BackendError::Unavailable(m),
        BusError::Cancelled => BackendError::Failed("cancelled".into()),
        BusError::Failed(m) if m.to_ascii_lowercase().contains("offline") => BackendError::Offline,
        BusError::Failed(m) => BackendError::Failed(m),
    }
}

/// Resolver problems -> error. Empty slice means no problem.
pub fn problems_to_error(problems: &[String]) -> Option<InstallError> {
    if problems.is_empty() {
        return None;
    }
    // Unknown keeps the real cause text in its detail.
    Some(failure_text(&problems.join("; ")))
}

fn is_removal(action: &str) -> bool {
    matches!(action, "Remove" | "Replaced" | "Removed" | "Obsoleted")
}

/// Extra packages an install would add: all installs/upgrades minus the file itself.
pub fn plan_from(res: &Resolution) -> InstallPlan {
    let adds = res.items.iter().filter(|i| !is_removal(&i.action)).count();
    InstallPlan {
        extra_packages: u32::try_from(adds.saturating_sub(1)).unwrap_or(u32::MAX),
        download_bytes: res.items.iter().map(|i| i.download_size).sum(),
    }
}

/// Extra packages a removal would take with it (v2 `removal_plan`).
pub fn removal_extra(res: &Resolution) -> u32 {
    let n = res.items.iter().filter(|i| is_removal(&i.action)).count();
    u32::try_from(n.saturating_sub(1)).unwrap_or(u32::MAX)
}

/// Newest installed version as `[epoch:]version-release`. ⚠ format must match
/// what the rpm inspector (S2b) produces so "already installed/update" compares.
pub fn installed_version(list: &[InstalledRpm]) -> Option<Version> {
    list.iter()
        .map(|r| {
            let s = if r.epoch > 0 {
                format!("{}:{}-{}", r.epoch, r.version, r.release)
            } else {
                format!("{}-{}", r.version, r.release)
            };
            Version::from(s.as_str())
        })
        .max()
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

fn pct(done: u64, total: u64) -> Option<u32> {
    if total == 0 {
        return None;
    }
    Some(u32::try_from(done.min(total).saturating_mul(100) / total).unwrap_or(100))
}

/// Folds signals into `InstallProgress`. Phase never goes backwards (D-019).
#[derive(Debug)]
pub struct ProgressTracker {
    phase: InstallPhase,
    downloads: Vec<(String, u64, u64)>,
    last: Option<InstallProgress>,
}

impl Default for ProgressTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgressTracker {
    pub fn new() -> Self {
        Self {
            phase: InstallPhase::Preparing,
            downloads: Vec::new(),
            last: None,
        }
    }

    /// Current phase.
    pub fn phase(&self) -> InstallPhase {
        self.phase
    }

    /// Force a phase (e.g. `WaitingForPermission` right before the polkit call).
    /// Returns progress only when it changed.
    pub fn enter(&mut self, phase: InstallPhase) -> Option<InstallProgress> {
        if rank(phase) > rank(self.phase) {
            self.phase = phase;
        }
        self.emit(InstallProgress::phase(self.phase))
    }

    pub fn on_signal(&mut self, sig: &Dnf5Signal) -> Option<InstallProgress> {
        match sig {
            Dnf5Signal::DownloadAdded { id, total } => {
                self.set_download(id, *total, 0);
                self.bump(InstallPhase::Downloading, self.download_pct())
            }
            Dnf5Signal::DownloadProgress {
                id,
                total,
                downloaded,
            } => {
                self.set_download(id, *total, *downloaded);
                self.bump(InstallPhase::Downloading, self.download_pct())
            }
            Dnf5Signal::TransactionStarted { .. } => self.bump(InstallPhase::Installing, Some(0)),
            Dnf5Signal::ElemProgress { processed, total } => {
                self.bump(InstallPhase::Installing, pct(*processed, *total))
            }
            Dnf5Signal::TransactionDone { .. } => self.bump(InstallPhase::Finishing, None),
        }
    }

    fn set_download(&mut self, id: &str, total: u64, done: u64) {
        match self.downloads.iter_mut().find(|d| d.0 == id) {
            Some(d) => (d.1, d.2) = (total, done),
            None => self.downloads.push((id.to_owned(), total, done)),
        }
    }

    fn download_pct(&self) -> Option<u32> {
        let total = self.downloads.iter().map(|d| d.1).sum();
        pct(self.downloads.iter().map(|d| d.2).sum(), total)
    }

    fn bump(&mut self, phase: InstallPhase, percent: Option<u32>) -> Option<InstallProgress> {
        if rank(phase) < rank(self.phase) {
            return None; // late signal from an earlier stage
        }
        self.phase = phase;
        self.emit(InstallProgress::with_percent(
            phase,
            percent.unwrap_or(101),
            None,
        ))
    }

    fn emit(&mut self, p: InstallProgress) -> Option<InstallProgress> {
        if self.last == Some(p) {
            return None;
        }
        self.last = Some(p);
        Some(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dnf5daemon::bus::ResolvedItem;

    fn item(a: &str, sz: u64) -> ResolvedItem {
        ResolvedItem {
            action: a.into(),
            download_size: sz,
        }
    }

    #[test]
    fn classify() {
        assert_eq!(
            classify_dbus_error("org.freedesktop.PolicyKit1.Error.NotAuthorized", ""),
            BusError::AccessDenied
        );
        assert_eq!(
            classify_dbus_error("org.freedesktop.DBus.Error.Failed", "Not authorized"),
            BusError::AccessDenied
        );
        // A repo 401 is a failure, not a cancel.
        assert_eq!(
            classify_dbus_error(
                "org.freedesktop.DBus.Error.Failed",
                "authentication required (401)"
            ),
            BusError::Failed("authentication required (401)".into())
        );
        assert!(matches!(
            classify_dbus_error(
                "org.freedesktop.DBus.Error.Spawn.ChildExited",
                "Process org.rpm.dnf.v0 exited with status 1"
            ),
            BusError::Unavailable(_)
        ));
        assert!(matches!(
            classify_dbus_error("org.freedesktop.DBus.Error.ServiceUnknown", ""),
            BusError::Unavailable(_)
        ));
        assert_eq!(
            classify_dbus_error("x.Y", "Operation was cancelled"),
            BusError::Cancelled
        );
        assert_eq!(
            classify_dbus_error("x.Y", "boom"),
            BusError::Failed("boom".into())
        );
    }

    #[test]
    fn denied_is_cancel_not_error() {
        assert_eq!(
            bus_to_install(BusError::AccessDenied),
            InstallError::Cancelled
        );
        assert_eq!(bus_to_install(BusError::Cancelled), InstallError::Cancelled);
        assert_eq!(
            bus_to_install(BusError::Unavailable("x".into())),
            InstallError::BackendUnavailable
        );
        let f = |s: &str| bus_to_install(BusError::Failed(s.into()));
        assert_eq!(f("file conflicts"), InstallError::Conflicts);
        assert_eq!(f("Cannot download repo"), InstallError::Offline);
        assert_eq!(f("GPG check failed"), InstallError::UntrustedSource);
        assert!(matches!(f("zzz"), InstallError::Unknown(_)));
        assert_eq!(
            bus_to_backend(BusError::AccessDenied),
            BackendError::NotAuthorized
        );
    }

    #[test]
    fn problems() {
        assert_eq!(problems_to_error(&[]), None);
        assert_eq!(
            problems_to_error(&["nothing provides libfoo".into()]),
            Some(InstallError::MissingDependencies)
        );
        assert_eq!(
            problems_to_error(&["weird cause".into()]),
            Some(InstallError::Unknown("weird cause".into()))
        );
        assert_eq!(
            problems_to_error(&["Problem: conflicting requests - nothing provides x".into()]),
            Some(InstallError::MissingDependencies)
        );
        assert_eq!(
            problems_to_error(&["file conflict".into()]),
            Some(InstallError::Conflicts)
        );
    }

    #[test]
    fn plans() {
        let r = Resolution {
            items: vec![
                item("Install", 0),
                item("Install", 100),
                item("Install", 50),
            ],
            problems: vec![],
        };
        assert_eq!(
            plan_from(&r),
            InstallPlan {
                extra_packages: 2,
                download_bytes: 150
            }
        );
        assert_eq!(plan_from(&Resolution::default()).extra_packages, 0);
        let rm = Resolution {
            items: vec![item("Remove", 0), item("Remove", 0)],
            problems: vec![],
        };
        assert_eq!(removal_extra(&rm), 1);
        assert_eq!(plan_from(&rm).extra_packages, 0);
    }

    #[test]
    fn versions() {
        assert_eq!(installed_version(&[]), None);
        let l = vec![
            InstalledRpm {
                epoch: 0,
                version: "1.0".into(),
                release: "1.fc45".into(),
            },
            InstalledRpm {
                epoch: 0,
                version: "1.2".into(),
                release: "1.fc45".into(),
            },
        ];
        assert_eq!(
            installed_version(&l).map(|v| v.to_string()),
            Some("1.2-1.fc45".into())
        );
        let e = [InstalledRpm {
            epoch: 2,
            version: "1".into(),
            release: "3".into(),
        }];
        assert_eq!(
            installed_version(&e).map(|v| v.to_string()),
            Some("2:1-3".into())
        );
    }

    #[test]
    fn progress_order_and_dedupe() {
        let mut t = ProgressTracker::new();
        let p = t.enter(InstallPhase::WaitingForPermission).unwrap();
        assert_eq!(p.phase, InstallPhase::WaitingForPermission);
        assert_eq!(t.enter(InstallPhase::WaitingForPermission), None);
        assert_eq!(t.enter(InstallPhase::Preparing), None);
        let p = t
            .on_signal(&Dnf5Signal::DownloadAdded {
                id: "a".into(),
                total: 200,
            })
            .unwrap();
        assert_eq!((p.phase, p.percent), (InstallPhase::Downloading, Some(0)));
        let dl = |downloaded| Dnf5Signal::DownloadProgress {
            id: "a".into(),
            total: 200,
            downloaded,
        };
        assert_eq!(t.on_signal(&dl(50)).unwrap().percent, Some(25));
        let p = t
            .on_signal(&Dnf5Signal::TransactionStarted { total: 2 })
            .unwrap();
        assert_eq!(p.phase, InstallPhase::Installing);
        assert_eq!(t.on_signal(&dl(200)), None); // late download signal
        let p = t
            .on_signal(&Dnf5Signal::ElemProgress {
                processed: 1,
                total: 2,
            })
            .unwrap();
        assert_eq!(p.percent, Some(50));
        let p = t
            .on_signal(&Dnf5Signal::TransactionDone { success: true })
            .unwrap();
        assert_eq!((p.phase, p.percent), (InstallPhase::Finishing, None));
    }

    #[test]
    fn zero_total_is_indeterminate() {
        let mut t = ProgressTracker::new();
        let p = t
            .on_signal(&Dnf5Signal::DownloadAdded {
                id: "a".into(),
                total: 0,
            })
            .unwrap();
        assert_eq!(p.percent, None);
    }
}
