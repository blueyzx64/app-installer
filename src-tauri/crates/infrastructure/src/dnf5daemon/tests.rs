//! Flow tests with a fake bus. No D-Bus needed.

use std::path::PathBuf;
use std::sync::Mutex;

use installer_application::CancelToken;
use installer_application::ports::{InstallObserver, SystemPackages};
use installer_domain::{
    BackendError, InstallError, InstallPhase, InstallPlan, InstallProgress, PackageFile,
    PackageFormat,
};
use tokio::sync::Notify;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use super::*;

#[derive(Default)]
struct FakeBus {
    calls: Mutex<Vec<String>>,
    installed: Vec<InstalledRpm>,
    resolution: Resolution,
    signals: Vec<Dnf5Signal>,
    tx_result: Option<BusError>,
    open_err: Option<BusError>,
    /// do_transaction waits until cancel() and then fails with Cancelled.
    block: bool,
    notify: Notify,
    sender: Mutex<Option<UnboundedSender<Dnf5Signal>>>,
}

impl FakeBus {
    fn log(&self, s: &str) {
        self.calls.lock().unwrap().push(s.to_owned());
    }
    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

impl Dnf5Bus for FakeBus {
    async fn open_session(&self) -> Result<SessionId, BusError> {
        self.log("open");
        match &self.open_err {
            Some(e) => Err(e.clone()),
            None => Ok(SessionId("/s1".into())),
        }
    }
    async fn close_session(&self, _: &SessionId) -> Result<(), BusError> {
        self.log("close");
        Ok(())
    }
    async fn list_installed(&self, _: &SessionId, n: &str) -> Result<Vec<InstalledRpm>, BusError> {
        self.log(&format!("list {n}"));
        Ok(self.installed.clone())
    }
    async fn queue_install(&self, _: &SessionId, p: &str) -> Result<(), BusError> {
        self.log(&format!("install {p}"));
        Ok(())
    }
    async fn queue_remove(&self, _: &SessionId, n: &str) -> Result<(), BusError> {
        self.log(&format!("remove {n}"));
        Ok(())
    }
    async fn resolve(&self, _: &SessionId) -> Result<Resolution, BusError> {
        self.log("resolve");
        Ok(self.resolution.clone())
    }
    async fn subscribe(&self, _: &SessionId) -> Result<UnboundedReceiver<Dnf5Signal>, BusError> {
        self.log("subscribe");
        let (tx, rx) = unbounded_channel();
        *self.sender.lock().unwrap() = Some(tx);
        Ok(rx)
    }
    async fn do_transaction(&self, _: &SessionId) -> Result<(), BusError> {
        self.log("do_transaction");
        let tx = self.sender.lock().unwrap().clone().unwrap();
        for s in &self.signals {
            tx.send(s.clone()).unwrap();
        }
        if self.block {
            self.notify.notified().await;
            return Err(BusError::Cancelled);
        }
        match &self.tx_result {
            Some(e) => Err(e.clone()),
            None => Ok(()),
        }
    }
    async fn cancel(&self, _: &SessionId) -> Result<(), BusError> {
        self.log("cancel");
        self.notify.notify_one();
        Ok(())
    }
}

#[derive(Default)]
struct Rec {
    progress: Mutex<Vec<InstallProgress>>,
    plans: Mutex<Vec<InstallPlan>>,
}
impl InstallObserver for Rec {
    fn on_progress(&self, p: InstallProgress) {
        self.progress.lock().unwrap().push(p);
    }
    fn on_plan(&self, p: &InstallPlan) {
        self.plans.lock().unwrap().push(*p);
    }
}
impl Rec {
    fn phases(&self) -> Vec<InstallPhase> {
        let mut v: Vec<_> = self
            .progress
            .lock()
            .unwrap()
            .iter()
            .map(|p| p.phase)
            .collect();
        v.dedup();
        v
    }
}

fn file() -> PackageFile {
    PackageFile {
        path: PathBuf::from("/tmp/a.rpm"),
        format: PackageFormat::Rpm,
        size_bytes: 1,
    }
}

fn two_installs() -> Resolution {
    Resolution {
        items: vec![
            ResolvedItem {
                action: "Install".into(),
                download_size: 0,
            },
            ResolvedItem {
                action: "Install".into(),
                download_size: 900,
            },
        ],
        problems: vec![],
    }
}

fn happy_signals() -> Vec<Dnf5Signal> {
    vec![
        Dnf5Signal::DownloadAdded {
            id: "d".into(),
            total: 100,
        },
        Dnf5Signal::DownloadProgress {
            id: "d".into(),
            total: 100,
            downloaded: 100,
        },
        Dnf5Signal::TransactionStarted { total: 2 },
        Dnf5Signal::ElemProgress {
            processed: 2,
            total: 2,
        },
        Dnf5Signal::TransactionDone { success: true },
    ]
}

#[tokio::test]
async fn install_happy_path_orders_phases_and_closes_session() {
    let bus = FakeBus {
        resolution: two_installs(),
        signals: happy_signals(),
        ..Default::default()
    };
    let d = Dnf5Daemon::new(bus);
    let rec = Rec::default();
    d.install(&file(), &rec, CancelToken::new()).await.unwrap();
    assert_eq!(
        rec.phases(),
        vec![
            InstallPhase::Preparing,
            InstallPhase::WaitingForPermission,
            InstallPhase::Downloading,
            InstallPhase::Installing,
            InstallPhase::Finishing,
        ]
    );
    assert_eq!(rec.plans.lock().unwrap()[0].extra_packages, 1);
    let calls = d.bus.calls();
    assert_eq!(calls.first().unwrap(), "open");
    assert_eq!(calls.last().unwrap(), "close");
    let pos = |s: &str| calls.iter().position(|c| c == s).unwrap();
    assert!(pos("subscribe") < pos("do_transaction"));
    assert!(pos("resolve") < pos("subscribe"));
}

#[tokio::test]
async fn polkit_dismissed_is_cancelled() {
    let bus = FakeBus {
        resolution: two_installs(),
        tx_result: Some(BusError::AccessDenied),
        ..Default::default()
    };
    let d = Dnf5Daemon::new(bus);
    let rec = Rec::default();
    let r = d.install(&file(), &rec, CancelToken::new()).await;
    assert_eq!(r, Err(InstallError::Cancelled));
    assert!(rec.phases().contains(&InstallPhase::WaitingForPermission));
    assert_eq!(d.bus.calls().last().unwrap(), "close");
}

#[tokio::test]
async fn resolve_problem_is_missing_deps_and_never_runs_transaction() {
    let bus = FakeBus {
        resolution: Resolution {
            items: vec![],
            problems: vec!["nothing provides libx".into()],
        },
        ..Default::default()
    };
    let d = Dnf5Daemon::new(bus);
    let r = d
        .install(&file(), &Rec::default(), CancelToken::new())
        .await;
    assert_eq!(r, Err(InstallError::MissingDependencies));
    assert!(!d.bus.calls().contains(&"do_transaction".to_owned()));
    assert_eq!(d.bus.calls().last().unwrap(), "close");
}

#[tokio::test]
async fn transaction_failure_maps() {
    let bus = FakeBus {
        resolution: two_installs(),
        tx_result: Some(BusError::Failed("Failed to download metadata".into())),
        ..Default::default()
    };
    let d = Dnf5Daemon::new(bus);
    let r = d
        .install(&file(), &Rec::default(), CancelToken::new())
        .await;
    assert_eq!(r, Err(InstallError::Offline));
}

#[tokio::test]
async fn cancel_token_cancels_goal() {
    let bus = FakeBus {
        resolution: two_installs(),
        block: true,
        ..Default::default()
    };
    let d = Dnf5Daemon::new(bus);
    let token = CancelToken::new();
    let t2 = token.clone();
    tokio::spawn(async move {
        tokio::task::yield_now().await;
        t2.cancel();
    });
    let r = d.install(&file(), &Rec::default(), token).await;
    assert_eq!(r, Err(InstallError::Cancelled));
    assert!(d.bus.calls().contains(&"cancel".to_owned()));
    assert_eq!(d.bus.calls().last().unwrap(), "close");
}

#[tokio::test]
async fn already_cancelled_skips_transaction() {
    let bus = FakeBus {
        resolution: two_installs(),
        ..Default::default()
    };
    let d = Dnf5Daemon::new(bus);
    let token = CancelToken::new();
    token.cancel();
    let r = d.install(&file(), &Rec::default(), token).await;
    assert_eq!(r, Err(InstallError::Cancelled));
    assert!(!d.bus.calls().contains(&"do_transaction".to_owned()));
}

#[tokio::test]
async fn bus_down_is_backend_unavailable() {
    let bus = FakeBus {
        open_err: Some(BusError::Unavailable("x".into())),
        ..Default::default()
    };
    let d = Dnf5Daemon::new(bus);
    let r = d
        .install(&file(), &Rec::default(), CancelToken::new())
        .await;
    assert_eq!(r, Err(InstallError::BackendUnavailable));
    assert!(matches!(
        d.installed_version("a").await,
        Err(BackendError::Unavailable(_))
    ));
}

#[tokio::test]
async fn plan_counts_extras_and_bytes() {
    let d = Dnf5Daemon::new(FakeBus {
        resolution: two_installs(),
        ..Default::default()
    });
    let p = d.plan(&file()).await.unwrap();
    assert_eq!(
        p,
        InstallPlan {
            extra_packages: 1,
            download_bytes: 900
        }
    );
    assert_eq!(d.bus.calls().last().unwrap(), "close");
}

#[tokio::test]
async fn plan_problem_fails() {
    let bus = FakeBus {
        resolution: Resolution {
            items: vec![],
            problems: vec!["x".into()],
        },
        ..Default::default()
    };
    let d = Dnf5Daemon::new(bus);
    assert!(matches!(
        d.plan(&file()).await,
        Err(BackendError::Failed(_))
    ));
}

#[tokio::test]
async fn installed_version_none_and_some() {
    let d = Dnf5Daemon::new(FakeBus::default());
    assert_eq!(d.installed_version("a").await.unwrap(), None);
    let bus = FakeBus {
        installed: vec![InstalledRpm {
            epoch: 0,
            version: "2.0".into(),
            release: "1".into(),
        }],
        ..Default::default()
    };
    let v = Dnf5Daemon::new(bus).installed_version("a").await.unwrap();
    assert_eq!(v.unwrap().to_string(), "2.0-1");
}

#[tokio::test]
async fn remove_and_removal_plan() {
    let res = Resolution {
        items: vec![
            ResolvedItem {
                action: "Remove".into(),
                download_size: 0,
            },
            ResolvedItem {
                action: "Remove".into(),
                download_size: 0,
            },
            ResolvedItem {
                action: "Remove".into(),
                download_size: 0,
            },
        ],
        problems: vec![],
    };
    let d = Dnf5Daemon::new(FakeBus {
        resolution: res,
        signals: vec![Dnf5Signal::TransactionStarted { total: 3 }],
        ..Default::default()
    });
    assert_eq!(d.removal_plan("lumen").await.unwrap(), 2);
    let rec = Rec::default();
    d.remove("lumen", &rec).await.unwrap();
    assert!(rec.plans.lock().unwrap().is_empty());
    assert!(d.bus.calls().contains(&"remove lumen".to_owned()));
    assert_eq!(d.bus.calls().last().unwrap(), "close");
}
