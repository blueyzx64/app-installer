//! R5 proofs for the dnf5daemon adapter over a REAL (private) D-Bus daemon, against
//! a fake `org.rpm.dnf.v0` server written from the upstream XML (dbus/interfaces/*.xml).
//! Needs `dbus-daemon`. Run:
//! `cargo test -p installer-infrastructure --features dnf5daemon --test review_r5_dnf5 -- --test-threads=1 --nocapture`
#![cfg(feature = "dnf5daemon")]

use std::collections::HashMap;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use installer_application::CancelToken;
use installer_application::ports::SystemPackages;
use installer_application::testing::RecordingObserver;
use installer_domain::{InstallError, InstallPhase, PackageFile, PackageFormat};
use installer_infrastructure::dnf5daemon::{Dnf5Daemon, ZbusBus};
use zbus::fdo;
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue};
use zbus::{Connection, connection::Builder};

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Ok,
    Denied,
}

#[derive(Default)]
struct St {
    log: Mutex<Vec<String>>,
    mode: Mutex<Option<Mode>>,
}

impl St {
    fn log(&self, s: impl Into<String>) {
        self.log.lock().unwrap().push(s.into());
    }
    fn calls(&self) -> Vec<String> {
        self.log.lock().unwrap().clone()
    }
}

type Attrs = HashMap<String, OwnedValue>;

struct Mgr(Arc<St>, std::sync::atomic::AtomicUsize);
#[zbus::interface(name = "org.rpm.dnf.v0.SessionManager")]
impl Mgr {
    fn open_session(&self, _options: Attrs) -> OwnedObjectPath {
        self.0.log("open_session");
        let n = self.1.fetch_add(1, std::sync::atomic::Ordering::SeqCst) % 8 + 1;
        OwnedObjectPath::try_from(format!("/org/rpm/dnf/v0/session_{n}")).unwrap()
    }
    fn close_session(&self, _p: ObjectPath<'_>) -> bool {
        self.0.log("close_session");
        true
    }
}

struct Rpm(Arc<St>);
#[zbus::interface(name = "org.rpm.dnf.v0.rpm.Rpm")]
impl Rpm {
    fn install(&self, specs: Vec<String>, _options: Attrs) {
        self.0.log(format!("install {specs:?}"));
    }
    fn remove(&self, specs: Vec<String>, _options: Attrs) {
        self.0.log(format!("remove {specs:?}"));
    }
    fn list(&self, _options: Attrs) -> Vec<Attrs> {
        vec![]
    }
}

struct Goal(Arc<St>, String);
type Item = (String, String, String, Attrs, Attrs);
#[zbus::interface(name = "org.rpm.dnf.v0.Goal")]
impl Goal {
    fn resolve(&self, _options: Attrs) -> (Vec<Item>, u32) {
        self.0.log("resolve");
        let mut pkg: Attrs = HashMap::new();
        pkg.insert("download_size".into(), OwnedValue::from(1000u64));
        (
            vec![(
                "Package".into(),
                "Install".into(),
                "User".into(),
                HashMap::new(),
                pkg,
            )],
            0,
        )
    }
    async fn do_transaction(
        &self,
        _options: Attrs,
        #[zbus(connection)] c: &Connection,
    ) -> fdo::Result<()> {
        self.0.log("do_transaction");
        if *self.0.mode.lock().unwrap() == Some(Mode::Denied) {
            // what sdbus-c++ produces for `throw std::runtime_error("Not authorized")`
            return Err(fdo::Error::Failed("Not authorized".into()));
        }
        let me = self.1.as_str();
        let sp = ObjectPath::try_from(me).unwrap();
        c.emit_signal(
            None::<&str>,
            me,
            "org.rpm.dnf.v0.Base",
            "download_add_new",
            &(&sp, "d1", "pkg", 1000i64),
        )
        .await?;
        c.emit_signal(
            None::<&str>,
            me,
            "org.rpm.dnf.v0.Base",
            "download_progress",
            &(&sp, "d1", 1000i64, 1000i64),
        )
        .await?;
        c.emit_signal(
            None::<&str>,
            me,
            "org.rpm.dnf.v0.rpm.Rpm",
            "transaction_before_begin",
            &(&sp, 1u64),
        )
        .await?;
        c.emit_signal(
            None::<&str>,
            me,
            "org.rpm.dnf.v0.rpm.Rpm",
            "transaction_elem_progress",
            &(&sp, "hello-1", 1u64, 1u64),
        )
        .await?;
        c.emit_signal(
            None::<&str>,
            me,
            "org.rpm.dnf.v0.rpm.Rpm",
            "transaction_after_complete",
            &(&sp, true),
        )
        .await?;
        tokio::time::sleep(Duration::from_millis(20)).await;
        Ok(())
    }
    fn cancel(&self) -> (bool, String) {
        self.0.log("cancel");
        (true, String::new())
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

async fn setup(mode: Mode) -> (PrivateBus, Arc<St>, Connection, Dnf5Daemon<ZbusBus>) {
    let bus = PrivateBus::start();
    // SAFETY: --test-threads=1
    unsafe { std::env::set_var("DBUS_SYSTEM_BUS_ADDRESS", &bus.addr) };
    let st = Arc::new(St::default());
    *st.mode.lock().unwrap() = Some(mode);
    let mut b = Builder::address(bus.addr.as_str())
        .unwrap()
        .name("org.rpm.dnf.v0")
        .unwrap()
        .serve_at("/org/rpm/dnf/v0", Mgr(st.clone(), Default::default()))
        .unwrap();
    for n in 1..=8 {
        let p = format!("/org/rpm/dnf/v0/session_{n}");
        b = b
            .serve_at(p.clone(), Rpm(st.clone()))
            .unwrap()
            .serve_at(p.clone(), Goal(st.clone(), p))
            .unwrap();
    }
    let server = b.build().await.unwrap();
    let d = Dnf5Daemon::connect().await.unwrap();
    (bus, st, server, d)
}

fn file() -> PackageFile {
    PackageFile {
        path: "/home/u/Downloads/hello-1.0-1.fc45.x86_64.rpm".into(),
        format: PackageFormat::Rpm,
        size_bytes: 1,
    }
}

/// Wire types (a{sv}, o, as, (a(sssa{sv}a{sv}),u), signals o/s/x/t/b) match upstream XML,
/// local .rpm path goes through Rpm.install, D-019 order holds, session closed.
#[tokio::test(flavor = "multi_thread")]
async fn review_r5_wire_happy_path_matches_upstream_xml() {
    let (_bus, st, _srv, d) = setup(Mode::Ok).await;
    let obs = RecordingObserver::default();
    let r = d.install(&file(), &obs, CancelToken::new()).await;
    println!(
        "calls {:?}\nphases {:?}\nplans {:?}",
        st.calls(),
        obs.phases(),
        obs.plans()
    );
    assert_eq!(r, Ok(()));
    let c = st.calls();
    assert!(
        c.iter()
            .any(|x| x.starts_with("install [\"/home/u/Downloads/hello"))
    );
    assert_eq!(c.first().unwrap(), "open_session");
    assert_eq!(c.last().unwrap(), "close_session");
    let ph = obs.phases();
    assert_eq!(ph.first(), Some(&InstallPhase::Preparing));
    assert_eq!(ph.last(), Some(&InstallPhase::Finishing));
    assert!(ph.contains(&InstallPhase::Downloading) && ph.contains(&InstallPhase::Installing));
    assert_eq!(obs.plans()[0].download_bytes, 1000);
}

#[tokio::test(flavor = "multi_thread")]
async fn review_r5_polkit_denied_is_cancelled_and_session_closed() {
    let (_bus, st, _srv, d) = setup(Mode::Denied).await;
    let obs = RecordingObserver::default();
    let r = d.install(&file(), &obs, CancelToken::new()).await;
    println!("calls {:?} phases {:?}", st.calls(), obs.phases());
    assert_eq!(r, Err(InstallError::Cancelled));
    assert_eq!(st.calls().last().unwrap(), "close_session");
}

/// subscribe() spawns a task that only exits when it tries to forward a signal of
/// its own session after the receiver is gone. After the install finishes nothing
/// ever wakes it: one task + one bus match rule leaks per install/plan/remove.
#[tokio::test(flavor = "multi_thread")]
async fn review_r5_subscribe_task_does_not_leak_per_install() {
    let (_bus, _st, _srv, d) = setup(Mode::Ok).await;
    let m = tokio::runtime::Handle::current().metrics();
    let obs = RecordingObserver::default();
    d.install(&file(), &obs, CancelToken::new()).await.unwrap(); // warm-up
    tokio::time::sleep(Duration::from_millis(300)).await;
    let before = m.num_alive_tasks();
    for _ in 0..5 {
        d.install(&file(), &obs, CancelToken::new()).await.unwrap();
    }
    tokio::time::sleep(Duration::from_millis(500)).await;
    let after = m.num_alive_tasks();
    println!("alive tasks before={before} after 5 installs={after}");
    assert_eq!(
        after,
        before,
        "leaked {} tasks over 5 installs",
        after.saturating_sub(before)
    );
}
