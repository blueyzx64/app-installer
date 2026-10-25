//! R5 review proofs: backend selection (`AnySystemPackages::detect`) and the
//! "rpm with no backend" path. Run single-threaded (env vars are global):
//! `cargo test -p installer --test review_r5_selection -- --test-threads=1 --nocapture`

use std::io::Write;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use installer_application::CancelToken;
use installer_application::ports::SystemPackages;
use installer_application::testing::RecordingObserver;
use installer_domain::{InstallError, InstallOutcome, PackageFile, PackageFormat};
use installer_lib::fallbacks::AnySystemPackages;

static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn set_bus(addr: &str) {
    // SAFETY: tests run with --test-threads=1.
    unsafe { std::env::set_var("DBUS_SYSTEM_BUS_ADDRESS", addr) };
}

/// Run `detect()` the way composition does (blocking on the tauri runtime),
/// on a helper thread so a hang fails the test instead of blocking forever.
fn detect_with_timeout(secs: u64) -> Option<(AnySystemPackages, Duration)> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let t = Instant::now();
        let d = tauri::async_runtime::block_on(AnySystemPackages::detect());
        let _ = tx.send((d, t.elapsed()));
    });
    rx.recv_timeout(Duration::from_secs(secs)).ok()
}

fn kind(a: &AnySystemPackages) -> &'static str {
    match a {
        AnySystemPackages::PackageKit(_) => "PackageKit",
        AnySystemPackages::Dnf5(_) => "Dnf5",
        AnySystemPackages::Unavailable(_) => "Unavailable",
    }
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("crates/infrastructure/tests/fixtures/rpm/hello-world-1.2.3-4.fc45.x86_64.rpm")
}

/// Private dbus-daemon with activatable `services` = (bus name, Exec).
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

#[test]
fn review_r5_detect_no_system_bus_is_fast_and_unavailable() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    set_bus("unix:path=/nonexistent/no-bus");
    let (d, took) = detect_with_timeout(5).expect("detect hung with no bus");
    println!("R5 no-bus detect: {} in {took:?}", kind(&d));
    assert_eq!(kind(&d), "Unavailable");
    assert!(took < Duration::from_millis(500), "slow: {took:?}");
}

/// Socket exists, peer accepts then never speaks (wedged/stale bus mount).
/// `build_state` blocks on this before the event loop => app never opens.
#[test]
fn review_r5_detect_hung_bus_must_not_block_startup() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("hung.sock");
    let l = UnixListener::bind(&sock).unwrap();
    std::thread::spawn(move || {
        let mut keep = Vec::new();
        for s in l.incoming().flatten() {
            keep.push(s); // hold open, never reply
        }
    });
    set_bus(&format!("unix:path={}", sock.display()));
    let r = detect_with_timeout(5);
    assert!(
        r.is_some(),
        "detect() blocked >5s on an unresponsive system bus (no timeout)"
    );
}

#[test]
fn review_r5_detect_bus_without_pk_or_dnf5_is_unavailable() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let bus = PrivateBus::start(&[]);
    set_bus(&bus.addr);
    let (d, took) = detect_with_timeout(5).expect("hang");
    println!("R5 empty bus detect: {} in {took:?}", kind(&d));
    assert_eq!(kind(&d), "Unavailable");
}

/// PackageKit has a service file (activatable) but its binary is broken.
/// detect() picks PackageKit and never reaches the dnf5daemon fallback; the
/// install then fails with something that is not a friendly backendUnavailable.
#[test]
fn review_r5_activatable_but_broken_packagekit_should_fall_back() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let bus = PrivateBus::start(&[
        ("org.freedesktop.PackageKit", "/bin/false"),
        ("org.rpm.dnf.v0", "/bin/false"),
    ]);
    set_bus(&bus.addr);
    let (d, _) = detect_with_timeout(5).expect("hang");
    println!("R5 broken-PK detect: {}", kind(&d));
    let file = PackageFile {
        path: fixture(),
        format: PackageFormat::Rpm,
        size_bytes: 1,
    };
    let obs = RecordingObserver::default();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let r = tauri::async_runtime::block_on(d.install(&file, &obs, CancelToken::new()));
        let _ = tx.send(r);
    });
    let r = rx
        .recv_timeout(Duration::from_secs(30))
        .expect("install hung");
    println!("R5 broken-PK install result: {r:?}");
    assert_eq!(r, Err(InstallError::BackendUnavailable), "got {r:?}");
}

/// dnf5daemon only, activatable but broken: ZbusBus never activates at connect,
/// so Dnf5 is chosen; first call fails with a spawn error. Must be BackendUnavailable.
#[test]
fn review_r5_broken_dnf5_activation_maps_to_backend_unavailable() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let bus = PrivateBus::start(&[("org.rpm.dnf.v0", "/bin/false")]);
    set_bus(&bus.addr);
    let (d, _) = detect_with_timeout(5).expect("hang");
    println!("R5 broken-dnf5 detect: {}", kind(&d));
    let file = PackageFile {
        path: fixture(),
        format: PackageFormat::Rpm,
        size_bytes: 1,
    };
    let obs = RecordingObserver::default();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let r = tauri::async_runtime::block_on(d.install(&file, &obs, CancelToken::new()));
        let _ = tx.send(r);
    });
    let r = rx
        .recv_timeout(Duration::from_secs(30))
        .expect("install hung");
    println!("R5 broken-dnf5 install result: {r:?}");
    assert_eq!(r, Err(InstallError::BackendUnavailable), "got {r:?}");
}

/// Same wiring as composition::build_state (system = detect()), but with a Fedora
/// os-release so the container host is "RPM based". No bus => friendly failure, no hang.
#[test]
fn review_r5_rpm_install_with_no_backend_ends_in_backend_unavailable() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    use installer_application::PreviewRegistry;
    use installer_application::use_cases::{InstallPackage, OpenPackage};
    use installer_infrastructure::appimage_store::AppImageStore;
    use installer_infrastructure::desktop_entries::DesktopEntries;
    use installer_infrastructure::file_probe::FsFileProbe;
    use installer_infrastructure::host::OsReleaseProbe;
    use installer_infrastructure::inspect::{
        appimage::AppImageInspector, deb::DebInspector, rpm::RpmInspector,
    };
    use installer_infrastructure::ledger::JsonLedger;
    use installer_lib::composition::SystemClock;
    use installer_lib::fallbacks::{InspectorChain, NoInspector};
    use std::sync::Arc;

    set_bus("unix:path=/nonexistent/no-bus");
    let dir = tempfile::tempdir().unwrap();
    let osr = dir.path().join("os-release");
    std::fs::write(
        &osr,
        "ID=fedora\nID_LIKE=\"rhel fedora\"\nVERSION_ID=45\nNAME=Fedora\n",
    )
    .unwrap();
    let data = dir.path().to_path_buf();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let system = Arc::new(tauri::async_runtime::block_on(AnySystemPackages::detect()));
        let registry = Arc::new(PreviewRegistry::new());
        let ledger = Arc::new(JsonLedger::new(&data));
        let insp = Arc::new(InspectorChain::new(
            AppImageInspector,
            InspectorChain::new(
                RpmInspector::new(),
                InspectorChain::new(DebInspector::new(), NoInspector),
            ),
        ));
        let host = Arc::new(OsReleaseProbe::with_roots(vec![osr], vec![]));
        let open = OpenPackage::new(
            Arc::new(FsFileProbe),
            insp,
            host,
            system.clone(),
            ledger.clone(),
            registry.clone(),
        );
        let inst = InstallPackage::new(
            system,
            Arc::new(AppImageStore::new(data.clone())),
            Arc::new(DesktopEntries::new(&data)),
            ledger,
            Arc::new(SystemClock),
            registry,
        );
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let out = rt.block_on(async {
            let preview = open.execute(fixture()).await.expect("preview");
            println!("R5 preview: {:?} {:?}", preview.meta.name, preview.compat);
            let obs = RecordingObserver::default();
            let o = inst.execute(preview.id, &obs).await;
            (o, obs.phases())
        });
        let _ = tx.send(out);
    });
    let (out, phases) = rx
        .recv_timeout(Duration::from_secs(20))
        .expect("install hung");
    println!("R5 outcome {out:?} phases {phases:?}");
    assert_eq!(
        out,
        InstallOutcome::Failed(InstallError::BackendUnavailable)
    );
    let _ = std::io::stdout().flush();
}
