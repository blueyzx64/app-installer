//! R5 re-verification: attacks on the S4 detect() fix (700 ms budget + health probe).
//! `cargo test -p installer --test review_r5_reverify_selection -- --test-threads=1 --nocapture`

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use installer_application::CancelToken;
use installer_application::ports::SystemPackages;
use installer_application::testing::RecordingObserver;
use installer_domain::{InstallError, PackageFile, PackageFormat};
use installer_infrastructure::dnf5daemon::Dnf5Daemon;
use installer_lib::fallbacks::AnySystemPackages;
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue};

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn set_bus(addr: &str) {
    // SAFETY: tests hold ENV_LOCK.
    unsafe { std::env::set_var("DBUS_SYSTEM_BUS_ADDRESS", addr) };
}

fn kind(a: &AnySystemPackages) -> &'static str {
    match a {
        AnySystemPackages::PackageKit(_) => "PackageKit",
        AnySystemPackages::Dnf5(_) => "Dnf5",
        AnySystemPackages::Unavailable(_) => "Unavailable",
    }
}

fn detect() -> (AnySystemPackages, Duration) {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let t = Instant::now();
        let d = tauri::async_runtime::block_on(AnySystemPackages::detect());
        let _ = tx.send((d, t.elapsed()));
    });
    rx.recv_timeout(Duration::from_secs(10))
        .expect("detect hung")
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("crates/infrastructure/tests/fixtures/rpm/hello-world-1.2.3-4.fc45.x86_64.rpm")
}

struct PrivateBus {
    child: Child,
    addr: String,
    _dir: tempfile::TempDir,
}
impl PrivateBus {
    fn start(services: &[(&str, &str)]) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let sdir = dir.path().join("services");
        std::fs::create_dir(&sdir).unwrap();
        for (name, exec) in services {
            std::fs::write(
                sdir.join(format!("{name}.service")),
                format!("[D-BUS Service]\nName={name}\nExec={exec}\n"),
            )
            .unwrap();
        }
        let sock = dir.path().join("bus.sock");
        let conf = dir.path().join("bus.conf");
        std::fs::write(
            &conf,
            format!(
                r#"<busconfig><type>session</type><listen>unix:path={}</listen>
<auth>EXTERNAL</auth><servicedir>{}</servicedir>
<policy context="default"><allow send_destination="*" eavesdrop="true"/><allow eavesdrop="true"/><allow own="*"/><allow user="*"/></policy></busconfig>"#,
                sock.display(),
                sdir.display()
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

type Attrs = HashMap<String, OwnedValue>;
type Log = Arc<Mutex<Vec<String>>>;

struct Mgr(Log);
#[zbus::interface(name = "org.rpm.dnf.v0.SessionManager")]
impl Mgr {
    fn open_session(&self, _o: Attrs) -> OwnedObjectPath {
        self.0.lock().unwrap().push("open_session".into());
        OwnedObjectPath::try_from("/org/rpm/dnf/v0/session_1").unwrap()
    }
    fn close_session(&self, _p: ObjectPath<'_>) -> bool {
        self.0.lock().unwrap().push("close_session".into());
        true
    }
}

/// `Rpm.list` runs `session.fill_sack()` first (rpm.cpp:596 -> session.cpp
/// read_all_repos -> load_repos of ALL enabled repos). Takes seconds on a real
/// box, more on a cold cache or slow network.
struct Rpm(Log, Duration);
#[zbus::interface(name = "org.rpm.dnf.v0.rpm.Rpm")]
impl Rpm {
    async fn list(&self, _o: Attrs) -> Vec<Attrs> {
        self.0.lock().unwrap().push("list".into());
        tokio::time::sleep(self.1).await;
        vec![]
    }
}

/// PackageKit owner that never answers CreateTransaction.
struct HungPk;
#[zbus::interface(name = "org.freedesktop.PackageKit")]
impl HungPk {
    async fn create_transaction(&self) -> OwnedObjectPath {
        tokio::time::sleep(Duration::from_secs(120)).await;
        OwnedObjectPath::try_from("/x").unwrap()
    }
}

/// Fake dnf5daemon (and optional hung PackageKit) on its own thread + runtime.
fn serve(addr: String, list_delay: Duration, hung_pk: bool) -> Log {
    let log: Log = Arc::default();
    let l2 = log.clone();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async move {
            let _c = zbus::connection::Builder::address(addr.as_str())
                .unwrap()
                .name("org.rpm.dnf.v0")
                .unwrap()
                .serve_at("/org/rpm/dnf/v0", Mgr(l2.clone()))
                .unwrap()
                .serve_at("/org/rpm/dnf/v0/session_1", Rpm(l2.clone(), list_delay))
                .unwrap()
                .build()
                .await
                .unwrap();
            let _p = if hung_pk {
                Some(
                    zbus::connection::Builder::address(addr.as_str())
                        .unwrap()
                        .name("org.freedesktop.PackageKit")
                        .unwrap()
                        .serve_at("/org/freedesktop/PackageKit", HungPk)
                        .unwrap()
                        .build()
                        .await
                        .unwrap(),
                )
            } else {
                None
            };
            let _ = tx.send(());
            std::future::pending::<()>().await;
        });
    });
    rx.recv_timeout(Duration::from_secs(10)).unwrap();
    log
}

/// F-31 on a real Fedora without PackageKit: the probe is `installed_version`,
/// i.e. open_session + Rpm.list, and Rpm.list loads every repo first. Any box
/// where that takes > 700 ms (normal) silently gets the "unavailable" stub.
#[test]
fn review_r5_rv_slow_but_healthy_dnf5daemon_is_still_selected() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let bus = PrivateBus::start(&[]);
    set_bus(&bus.addr);
    let log = serve(bus.addr.clone(), Duration::from_millis(1500), false);
    let (d, took) = detect();
    println!(
        "R5rv slow dnf5 detect: {} in {took:?}; server {:?}",
        kind(&d),
        log.lock().unwrap()
    );
    assert_eq!(
        kind(&d),
        "Dnf5",
        "healthy dnf5daemon rejected by the 700 ms probe"
    );
}

/// The probe future is dropped on timeout after open_session succeeded:
/// close_session is never sent and the root daemon keeps a session (with a
/// loaded repo sack) alive for the rest of its life.
#[test]
fn review_r5_rv_probe_timeout_does_not_leak_a_daemon_session() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let bus = PrivateBus::start(&[]);
    set_bus(&bus.addr);
    let log = serve(bus.addr.clone(), Duration::from_millis(1500), false);
    let _ = detect();
    std::thread::sleep(Duration::from_millis(2500));
    let calls = log.lock().unwrap().clone();
    let opens = calls.iter().filter(|c| *c == "open_session").count();
    let closes = calls.iter().filter(|c| *c == "close_session").count();
    println!("R5rv server calls after probe timeout: {calls:?}");
    assert_eq!(
        opens, closes,
        "session opened by the probe was never closed"
    );
}

/// Fix check: hung PackageKit + fast dnf5daemon => dnf5 picked, within budget.
#[test]
fn review_r5_rv_hung_packagekit_falls_back_to_fast_dnf5() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let bus = PrivateBus::start(&[]);
    set_bus(&bus.addr);
    let _log = serve(bus.addr.clone(), Duration::from_millis(10), true);
    let (d, took) = detect();
    println!("R5rv hung PK + fast dnf5: {} in {took:?}", kind(&d));
    assert_eq!(kind(&d), "Dnf5");
    assert!(took < Duration::from_millis(1600), "took {took:?}");
}

/// Fix check for the activation mapping, bypassing detect(): Dnf5Daemon used
/// directly against an activatable-but-broken service.
#[test]
fn review_r5_rv_broken_dnf5_activation_direct_is_backend_unavailable() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let bus = PrivateBus::start(&[("org.rpm.dnf.v0", "/bin/false")]);
    set_bus(&bus.addr);
    let file = PackageFile {
        path: fixture(),
        format: PackageFormat::Rpm,
        size_bytes: 1,
    };
    let r = tauri::async_runtime::block_on(async {
        let d = Dnf5Daemon::connect().await.expect("connect");
        d.install(&file, &RecordingObserver::default(), CancelToken::new())
            .await
    });
    println!("R5rv broken dnf5 direct: {r:?}");
    assert_eq!(r, Err(InstallError::BackendUnavailable));
}
