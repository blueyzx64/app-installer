//! R5 re-verification: attacks on the S2c fixes. Fake `org.rpm.dnf.v0` written from
//! upstream dnf5 main (dbus/interfaces/*.xml, threads_manager.hpp, goal.cpp).
//! `cargo test -p installer-infrastructure --features dnf5daemon --test review_r5_reverify -- --test-threads=1 --nocapture`
#![cfg(feature = "dnf5daemon")]

use std::collections::HashMap;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use installer_application::CancelToken;
use installer_application::ports::{InstallObserver, SystemPackages};
use installer_application::testing::RecordingObserver;
use installer_domain::{BackendError, InstallError, PackageFile, PackageFormat};
use installer_infrastructure::dnf5daemon::{
    BusError, Dnf5Bus, Dnf5Daemon, Dnf5Signal, InstalledRpm, Resolution, ResolvedItem, SessionId,
    ZbusBus,
};
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
use zbus::connection::Builder;
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue};

static BUS_ENV: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

type Attrs = HashMap<String, OwnedValue>;
type Item = (String, String, String, Attrs, Attrs);

/// What dnf5daemon's threads_manager.hpp:85 sends for any std::exception:
/// `sdbus::Error("org.rpm.dnf.v0.Error", ex.what())`.
#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.rpm.dnf.v0")]
enum DnfErr {
    #[zbus(error)]
    ZBus(zbus::Error),
    Error(String),
}

#[derive(Default)]
struct St {
    log: Mutex<Vec<String>>,
    deny: Mutex<bool>,
}

struct Mgr(Arc<St>);
#[zbus::interface(name = "org.rpm.dnf.v0.SessionManager")]
impl Mgr {
    fn open_session(&self, _o: Attrs) -> OwnedObjectPath {
        self.0.log.lock().unwrap().push("open_session".into());
        OwnedObjectPath::try_from("/org/rpm/dnf/v0/session_1").unwrap()
    }
    fn close_session(&self, _p: ObjectPath<'_>) -> bool {
        self.0.log.lock().unwrap().push("close_session".into());
        true
    }
}

struct Rpm;
#[zbus::interface(name = "org.rpm.dnf.v0.rpm.Rpm")]
impl Rpm {
    fn install(&self, _s: Vec<String>, _o: Attrs) {}
    fn list(&self, _o: Attrs) -> Vec<Attrs> {
        vec![]
    }
}

struct Goal(Arc<St>);
#[zbus::interface(name = "org.rpm.dnf.v0.Goal")]
impl Goal {
    /// Resolve fails (result = 2, upstream "2 - resolving failed").
    fn resolve(&self, _o: Attrs) -> (Vec<Item>, u32) {
        if *self.0.deny.lock().unwrap() {
            let mut pkg: Attrs = HashMap::new();
            pkg.insert("download_size".into(), OwnedValue::from(0u64));
            return (
                vec![(
                    "Package".into(),
                    "Install".into(),
                    "User".into(),
                    HashMap::new(),
                    pkg,
                )],
                0,
            );
        }
        (vec![], 2)
    }
    /// Upstream Goal.xml: `<arg name="problems" type="as" direction="out"/>`.
    fn get_transaction_problems_string(&self) -> Vec<String> {
        self.0
            .log
            .lock()
            .unwrap()
            .push("get_transaction_problems_string".into());
        vec!["Problem: package hello-1.0-1.x86_64 conflicts with goodbye provided by goodbye-2.0-1.x86_64".into()]
    }
    fn do_transaction(&self, _o: Attrs) -> Result<(), DnfErr> {
        // goal.cpp:404 `throw std::runtime_error("Not authorized")`
        Err(DnfErr::Error("Not authorized".into()))
    }
    fn cancel(&self) -> (bool, String) {
        (false, "not running".into())
    }
}

struct PrivateBus {
    child: Child,
    addr: String,
    _dir: tempfile::TempDir,
}
impl PrivateBus {
    fn start() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("bus.sock");
        let conf = dir.path().join("bus.conf");
        std::fs::write(
            &conf,
            format!(
                r#"<busconfig><type>session</type><listen>unix:path={}</listen><auth>EXTERNAL</auth>
<policy context="default"><allow send_destination="*" eavesdrop="true"/><allow eavesdrop="true"/><allow own="*"/><allow user="*"/></policy></busconfig>"#,
                sock.display()
            ),
        )
        .unwrap();
        let child = Command::new("dbus-daemon")
            .arg("--config-file")
            .arg(&conf)
            .arg("--nofork")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("dbus-daemon");
        for _ in 0..50 {
            if sock.exists() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Self {
            child,
            addr: format!("unix:path={}", sock.display()),
            _dir: dir,
        }
    }
}
impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

async fn setup(deny: bool) -> (PrivateBus, Arc<St>, zbus::Connection, Dnf5Daemon<ZbusBus>) {
    let bus = PrivateBus::start();
    // SAFETY: tests hold BUS_ENV.
    unsafe { std::env::set_var("DBUS_SYSTEM_BUS_ADDRESS", &bus.addr) };
    let st = Arc::new(St::default());
    *st.deny.lock().unwrap() = deny;
    let p = "/org/rpm/dnf/v0/session_1";
    let srv = Builder::address(bus.addr.as_str())
        .unwrap()
        .name("org.rpm.dnf.v0")
        .unwrap()
        .serve_at("/org/rpm/dnf/v0", Mgr(st.clone()))
        .unwrap()
        .serve_at(p, Rpm)
        .unwrap()
        .serve_at(p, Goal(st.clone()))
        .unwrap()
        .build()
        .await
        .unwrap();
    let d = Dnf5Daemon::connect().await.unwrap();
    (bus, st, srv, d)
}

fn file() -> PackageFile {
    PackageFile {
        path: "/home/u/Downloads/hello-1.0-1.fc45.x86_64.rpm".into(),
        format: PackageFormat::Rpm,
        size_bytes: 1,
    }
}

/// Fix "resolve detail": proxy declares get_transaction_problems_string -> `s`,
/// upstream returns `as`. The call fails on the signature, unwrap_or_default
/// hides it, and the generic text comes back: the real cause is still lost.
#[tokio::test(flavor = "multi_thread")]
async fn review_r5_rv_resolve_problem_text_reaches_the_error() {
    let _env = BUS_ENV.lock().await;
    let (_bus, st, _srv, d) = setup(false).await;
    let plan = d.plan(&file()).await;
    println!("calls {:?}\nplan {plan:?}", st.log.lock().unwrap());
    let inst = d
        .install(&file(), &RecordingObserver::default(), CancelToken::new())
        .await;
    println!("install {inst:?}");
    match &plan {
        Err(BackendError::Failed(t)) => assert!(t.contains("conflicts with"), "lost cause: {t:?}"),
        other => panic!("unexpected {other:?}"),
    }
    assert_eq!(inst, Err(InstallError::Conflicts));
}

/// Real dnf5daemon error name for the polkit refusal is org.rpm.dnf.v0.Error.
#[tokio::test(flavor = "multi_thread")]
async fn review_r5_rv_polkit_denied_with_real_error_name_is_cancel() {
    let _env = BUS_ENV.lock().await;
    let (_bus, st, _srv, d) = setup(true).await;
    let r = d
        .install(&file(), &RecordingObserver::default(), CancelToken::new())
        .await;
    println!("calls {:?} r {r:?}", st.log.lock().unwrap());
    assert_eq!(r, Err(InstallError::Cancelled));
    assert_eq!(st.log.lock().unwrap().last().unwrap(), "close_session");
}

// ---- cancel latch, over a plain fake bus ----

struct LatchBus {
    cancels: Arc<AtomicUsize>,
    signals: usize,
}

impl Dnf5Bus for LatchBus {
    async fn open_session(&self) -> Result<SessionId, BusError> {
        Ok(SessionId("/s".into()))
    }
    async fn close_session(&self, _: &SessionId) -> Result<(), BusError> {
        Ok(())
    }
    async fn list_installed(&self, _: &SessionId, _: &str) -> Result<Vec<InstalledRpm>, BusError> {
        Ok(vec![])
    }
    async fn queue_install(&self, _: &SessionId, _: &str) -> Result<(), BusError> {
        Ok(())
    }
    async fn queue_remove(&self, _: &SessionId, _: &str) -> Result<(), BusError> {
        Ok(())
    }
    async fn resolve(&self, _: &SessionId) -> Result<Resolution, BusError> {
        Ok(Resolution {
            items: vec![ResolvedItem {
                action: "Install".into(),
                download_size: 0,
            }],
            problems: vec![],
        })
    }
    async fn subscribe(&self, _: &SessionId) -> Result<UnboundedReceiver<Dnf5Signal>, BusError> {
        // Local rpm, nothing to download: the daemon goes straight to the rpm
        // transaction, where Goal.cancel is NOT_ALLOWED (goal.cpp cancel()).
        let (tx, rx) = unbounded_channel();
        let n = self.signals as u64;
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await; // polkit dialog
            let _ = tx.send(Dnf5Signal::TransactionStarted { total: n });
            for i in 1..=n {
                let _ = tx.send(Dnf5Signal::ElemProgress {
                    processed: i,
                    total: n,
                });
                tokio::task::yield_now().await;
            }
            let _ = tx.send(Dnf5Signal::TransactionDone { success: true });
            std::mem::forget(tx);
        });
        Ok(rx)
    }
    async fn do_transaction(&self, _: &SessionId) -> Result<(), BusError> {
        tokio::time::sleep(Duration::from_millis(600)).await;
        Ok(())
    }
    async fn cancel(&self, _: &SessionId) -> Result<bool, BusError> {
        self.cancels.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(1)).await; // one bus round trip
        Ok(false)
    }
}

/// Cancel pressed during the permission dialog, daemon refuses every time
/// (rpm phase). The latch re-sends Goal.cancel on EVERY progress signal:
/// one blocking D-Bus round trip per package element, for the whole install.
#[tokio::test(flavor = "multi_thread")]
async fn review_r5_rv_cancel_latch_does_not_spam_the_daemon() {
    let cancels = Arc::new(AtomicUsize::new(0));
    let d = Dnf5Daemon::new(LatchBus {
        cancels: cancels.clone(),
        signals: 300,
    });
    let obs = RecordingObserver::default();
    let tok = CancelToken::new();
    let t2 = tok.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(20)).await;
        t2.cancel();
    });
    let r = d.install(&file(), &obs as &dyn InstallObserver, tok).await;
    let n = cancels.load(Ordering::SeqCst);
    println!("result {r:?}, Goal.cancel calls = {n}");
    assert!(n <= 3, "Goal.cancel sent {n} times");
}
