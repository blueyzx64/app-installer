//! R2 re-verification: attacks on the S1/S1b/S3 fixes. A failing `review_r2v_*` is a finding.
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use installer_application::PreviewRegistry;
use installer_application::ports::{AppLedger, DesktopIntegration, Launcher};
use installer_application::testing::*;
use installer_application::use_cases::{InstallPackage, ListInstalledApps};
use installer_domain::*;

use crate::appimage_store::AppImageStore;
use crate::desktop_entries::DesktopEntries;
use crate::inspect::appimage::inspect_path;
use crate::inspect::appimage::testkit::{Entry, Spec, build, desktop};
use crate::launcher::GioLauncher;
use crate::ledger::JsonLedger;

fn trash(p: &Path) -> io::Result<()> {
    fs::remove_file(p)
}

fn mk(dir: &Path, file: &str, name: &str, ver: &str) -> PackageFile {
    let bytes = build(Spec {
        entries: vec![Entry::File("/a.desktop", desktop(name, "icon", Some(ver)))],
        ..Spec::default()
    });
    let path = dir.join(file);
    fs::write(&path, &bytes).unwrap();
    PackageFile {
        path,
        format: PackageFormat::AppImage,
        size_bytes: bytes.len() as u64,
    }
}

struct World {
    _dirs: Vec<tempfile::TempDir>,
    src: PathBuf,
    data: PathBuf,
    store: Arc<AppImageStore>,
    de: Arc<DesktopEntries>,
    ledger: Arc<JsonLedger>,
    system: Arc<FakeSystemPackages>,
    registry: Arc<PreviewRegistry>,
}

impl World {
    fn new() -> Self {
        let (s, r, d) = (
            tempfile::tempdir().unwrap(),
            tempfile::tempdir().unwrap(),
            tempfile::tempdir().unwrap(),
        );
        World {
            src: s.path().to_path_buf(),
            data: d.path().to_path_buf(),
            store: Arc::new(AppImageStore::new(r.path()).with_trash(trash)),
            de: Arc::new(DesktopEntries::new(d.path())),
            ledger: Arc::new(JsonLedger::new(d.path())),
            system: Arc::new(FakeSystemPackages::default()),
            registry: Arc::new(PreviewRegistry::new()),
            _dirs: vec![s, r, d],
        }
    }

    async fn install(&self, file_name: &str, display: &str) -> InstallOutcome {
        let f = mk(&self.src, file_name, display, "1.0");
        let mut meta = inspect_path(&f.path).unwrap();
        meta.scope = InstallScope::User;
        let p = self.registry.insert(f.clone(), |id| PackagePreview {
            id,
            size_bytes: f.size_bytes,
            meta,
            compat: Compatibility::Ok,
            needs_permission: false,
            installed_app_id: None,
        });
        InstallPackage::new(
            self.system.clone(),
            self.store.clone(),
            self.de.clone(),
            self.ledger.clone(),
            Arc::new(FakeClock(1)),
            self.registry.clone(),
        )
        .execute(p.id, &RecordingObserver::default())
        .await
    }

    async fn list(&self) -> Vec<InstalledApp> {
        ListInstalledApps::new(
            self.store.clone(),
            self.ledger.clone(),
            self.system.clone(),
            self.de.clone(),
        )
        .execute()
        .await
        .unwrap()
    }

    fn desktop_files(&self) -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(self.data.join("applications"))
            .map(|d| {
                d.filter_map(Result::ok)
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .filter(|n| n.ends_with(".desktop"))
                    .collect()
            })
            .unwrap_or_default();
        v.sort();
        v
    }
}

/// "Foo Bar" and "Foo-Bar" are two apps (two files) but machine name == slug for AppImages,
/// so free_id's `a.name == name` treats them as the same app: one id, one menu entry.
#[tokio::test]
async fn review_r2v_same_slug_appimages_keep_both_entries() {
    let w = World::new();
    assert!(matches!(
        w.install("a.AppImage", "Foo Bar").await,
        InstallOutcome::Installed(_)
    ));
    assert!(matches!(
        w.install("b.AppImage", "Foo-Bar").await,
        InstallOutcome::Installed(_)
    ));
    let apps = w.list().await;
    assert_eq!(
        w.desktop_files().len(),
        2,
        "second app replaced the first menu entry: {:?}",
        w.desktop_files()
    );
    assert_eq!(apps.len(), 2, "library shows {} app(s)", apps.len());
}

/// An AppImage whose slug matches a recorded system package: names are equal slugs, so
/// free_id ignores `source` and ledger.record replaces the system package row.
#[tokio::test]
async fn review_r2v_appimage_does_not_replace_system_package_row() {
    let w = World::new();
    w.ledger
        .record(&sample_app("lumen-notes", "1", AppSource::SystemPackage))
        .await
        .unwrap();
    w.system.set_installed("lumen-notes", "1");
    assert!(matches!(
        w.install("a.AppImage", "Lumen Notes").await,
        InstallOutcome::Installed(_)
    ));
    let rows = w.ledger.list().await.unwrap();
    let sources: Vec<_> = rows.iter().map(|a| a.source).collect();
    assert!(
        sources.contains(&AppSource::SystemPackage) && sources.contains(&AppSource::AppImage),
        "rows: {sources:?}"
    );
}

/// When free_id DOES add a suffix (different machine name, same slug), scan still reports
/// the unsuffixed id, so the next list forgets the AppImage and deletes its menu entry.
#[tokio::test]
async fn review_r2v_suffixed_id_survives_reconcile() {
    let w = World::new();
    w.ledger
        .record(&sample_app("lumen_notes", "1", AppSource::SystemPackage))
        .await
        .unwrap();
    w.system.set_installed("lumen_notes", "1");
    let InstallOutcome::Installed(app) = w.install("a.AppImage", "Lumen Notes").await else {
        panic!("install failed");
    };
    assert_eq!(
        app.id.as_str(),
        "lumen-notes-2",
        "precondition: suffix applied"
    );
    let before = w.desktop_files();
    let apps = w.list().await;
    assert!(
        apps.iter().any(|a| a.source == AppSource::AppImage),
        "AppImage dropped from library: {:?}",
        apps.iter().map(|a| a.id.as_str()).collect::<Vec<_>>()
    );
    assert_eq!(w.desktop_files(), before, "menu entry deleted by reconcile");
}

/// The file exists but cannot be inspected (e.g. a newer AppImage format): reconcile now
/// also deletes its working menu entry, not just the ledger row.
#[tokio::test]
async fn review_r2v_uninspectable_file_keeps_menu_entry() {
    let w = World::new();
    let InstallOutcome::Installed(app) = w.install("a.AppImage", "Lumen Notes").await else {
        panic!("install failed");
    };
    let path = app.path.clone().unwrap();
    fs::write(&path, b"\x7fELF not a squashfs we can read").unwrap();
    assert!(path.exists());
    let _ = w.list().await;
    assert_eq!(
        w.desktop_files().len(),
        1,
        "menu entry for an existing file was removed"
    );
}

/// The sh wrapper used for paths with '%': real launch through GIO, hostile file name.
/// The program must run, get its own path as $0, no extra args, and no injected command.
#[tokio::test]
async fn review_r2v_sh_wrapper_launch_has_no_injection() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out");
    let pwn = dir.path().join("PWNED");
    let names = [
        "100% $(touch PWNED) `touch PWNED`.AppImage",
        "50%; touch PWNED; #.AppImage",
        "x% \" ' \\ $HOME %U %f %%.AppImage",
        "%.AppImage",
        "-c %.AppImage",
    ];
    for (i, n) in names.iter().enumerate() {
        let _ = fs::remove_file(&out);
        let p = dir.path().join(n);
        fs::write(
            &p,
            format!(
                "#!/bin/sh\ncd {d}\nprintf '%s\\n' \"$0\" \"$#\" > {o}\n",
                d = dir.path().display(),
                o = out.display()
            ),
        )
        .unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
        let apps_dir = dir.path().join("apps");
        let de = DesktopEntries::new(dir.path());
        let mut a = sample_app(&format!("t{i}"), "1", AppSource::AppImage);
        a.path = Some(p.clone());
        let id = de.install_entry(&a, None).await.unwrap();
        let _ = apps_dir;
        let mut la = a.clone();
        la.desktop_id = Some(id);
        let l = GioLauncher::with_applications_dir(dir.path().join("applications"));
        l.launch(&la).await.unwrap_or_else(|e| panic!("{n:?}: {e}"));
        let mut got = String::new();
        for _ in 0..50 {
            if let Ok(s) = fs::read_to_string(&out)
                && s.lines().count() >= 2
            {
                got = s;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        let lines: Vec<&str> = got.lines().collect();
        assert_eq!(
            lines.first().copied(),
            Some(p.to_str().unwrap()),
            "{n:?}: $0 = {got:?}"
        );
        assert_eq!(lines.get(1).copied(), Some("0"), "{n:?}: extra args");
        assert!(!pwn.exists(), "{n:?}: injection ran");
        assert!(!Path::new("PWNED").exists(), "{n:?}: injection ran in cwd");
    }
}
