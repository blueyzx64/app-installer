//! R2 adversarial proof tests. Each `review_r2_*` that FAILS is a finding.
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;

use installer_application::ports::{AppLedger, DesktopIntegration, UserApps};
use installer_application::testing::*;
use installer_application::use_cases::{ListInstalledApps, UninstallApp};
use installer_domain::*;

use crate::appimage_store::AppImageStore;
use crate::desktop_entries::{DesktopEntries, render_entry};
use crate::inspect::appimage::inspect_path;
use crate::inspect::appimage::testkit::{Entry, Spec, build, desktop};
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

fn meta_of(f: &PackageFile) -> PackageMetadata {
    let mut m = inspect_path(&f.path).unwrap();
    m.scope = InstallScope::User;
    m
}

fn app_from(meta: &PackageMetadata, path: &Path) -> InstalledApp {
    InstalledApp {
        id: AppId::from_name(&meta.name),
        name: meta.name.clone(),
        display_name: meta.display_name.clone(),
        version: meta.version.clone(),
        icon: None,
        source: AppSource::AppImage,
        installed_at: 0,
        desktop_id: None,
        path: Some(path.to_path_buf()),
    }
}

/// Non-ASCII names all slug to "app": distinct apps share id, desktop file, ledger row.
#[tokio::test]
async fn review_r2_non_ascii_names_get_distinct_ids() {
    let src = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let store = AppImageStore::new(root.path()).with_trash(trash);
    let de = DesktopEntries::new(data.path());
    let a = mk(src.path(), "a.AppImage", "\u{65e5}\u{672c}\u{8a9e}", "1.0");
    let b = mk(src.path(), "b.AppImage", "\u{4e2d}\u{6587}", "1.0");
    for f in [&a, &b] {
        let m = meta_of(f);
        let p = store.install(f, &m).await.unwrap();
        de.install_entry(&app_from(&m, &p), None).await.unwrap();
    }
    let n_desktop = fs::read_dir(data.path().join("applications"))
        .unwrap()
        .count();
    let apps = store.scan().await.unwrap();
    assert_eq!(apps.len(), 2);
    assert_ne!(
        apps[0].id, apps[1].id,
        "two different apps share one AppId {:?}",
        apps[0].id
    );
    assert_eq!(n_desktop, 2, "second app overwrote first menu entry");
}

/// "C++ Studio" and "C Studio" are two files but one AppId.
#[tokio::test]
async fn review_r2_same_slug_different_names_get_distinct_ids() {
    let src = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let store = AppImageStore::new(root.path()).with_trash(trash);
    for (n, f) in [("C++ Studio", "a"), ("C Studio", "b")] {
        let file = mk(src.path(), &format!("{f}.AppImage"), n, "1.0");
        store.install(&file, &meta_of(&file)).await.unwrap();
    }
    let apps = store.scan().await.unwrap();
    assert_eq!(apps.len(), 2);
    assert_ne!(apps[0].id, apps[1].id, "id collision {:?}", apps[0].id);
}

/// ~/Applications is a symlink (e.g. /home -> /var/home): in-place install records the
/// canonical path, trash then refuses it.
#[tokio::test]
async fn review_r2_symlinked_store_root_can_be_uninstalled() {
    let real = tempfile::tempdir().unwrap();
    let holder = tempfile::tempdir().unwrap();
    let link = holder.path().join("Applications");
    std::os::unix::fs::symlink(real.path(), &link).unwrap();
    let store = AppImageStore::new(&link).with_trash(trash);
    let f = mk(&link, "Here.AppImage", "Here", "1.0");
    let m = meta_of(&f);
    let path = store.install(&f, &m).await.unwrap();
    let app = app_from(&m, &path);
    assert_eq!(store.trash(&app).await, Ok(()), "path recorded: {path:?}");
}

/// AppImage opened from inside the store with a name scan ignores (no .AppImage ext).
#[tokio::test]
async fn review_r2_install_in_place_with_odd_name_is_listed() {
    let root = tempfile::tempdir().unwrap();
    let store = AppImageStore::new(root.path()).with_trash(trash);
    let f = mk(root.path(), "Here.bin", "Here", "1.0");
    let m = meta_of(&f);
    let path = store.install(&f, &m).await.unwrap();
    assert!(path.exists());
    let apps = store.scan().await.unwrap();
    assert_eq!(
        apps.len(),
        1,
        "installed in place, scan never sees it -> reconcile forgets it"
    );
}

/// Store root unreadable (here: a regular file) must not make reconcile forget apps.
#[tokio::test]
async fn review_r2_unreadable_store_makes_reconcile_forget_everything() {
    let dir = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let root = dir.path().join("Applications");
    fs::write(&root, b"i am a file, read_dir -> ENOTDIR").unwrap();
    let store = Arc::new(AppImageStore::new(&root).with_trash(trash));
    let ledger = Arc::new(JsonLedger::new(data.path()));
    ledger
        .record(&sample_app("keepme", "1", AppSource::AppImage))
        .await
        .unwrap();
    let uc = ListInstalledApps::new(
        store,
        ledger.clone(),
        Arc::new(FakeSystemPackages::default()),
    );
    let apps = uc.execute().await.unwrap();
    assert_eq!(apps.len(), 1, "app dropped on scan I/O error");
    assert_eq!(ledger.list().await.unwrap().len(), 1);
}

/// 99 foreign files fill every slot: the next install overwrites a stranger's file.
#[tokio::test]
async fn review_r2_suffix_exhaustion_never_overwrites_foreign_file() {
    let src = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let store = AppImageStore::new(root.path()).with_trash(trash);
    fs::write(root.path().join("Foo.AppImage"), b"foreign").unwrap();
    for n in 2..=100 {
        fs::write(root.path().join(format!("Foo ({n}).AppImage")), b"foreign").unwrap();
    }
    let f = mk(src.path(), "x.AppImage", "Foo", "1.0");
    let _ = store.install(&f, &meta_of(&f)).await;
    assert!(
        fs::read(root.path().join("Foo.AppImage")).unwrap() == b"foreign",
        "foreign Foo.AppImage was overwritten"
    );
}

/// ~/Applications exists as a file: error should be a clean one, not raw OS text.
#[tokio::test]
async fn review_r2_store_root_is_a_file() {
    let src = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Applications");
    fs::write(&root, b"x").unwrap();
    let store = AppImageStore::new(&root).with_trash(trash);
    let f = mk(src.path(), "x.AppImage", "Foo", "1.0");
    let r = store.install(&f, &meta_of(&f)).await;
    eprintln!("root-is-file -> {r:?}");
    assert!(r.is_err());
    assert!(
        !matches!(r, Err(InstallError::Unknown(_))),
        "raw OS error leaks as Unknown: {r:?}"
    );
}

/// Uninstall when the file was already deleted by hand: entry, icons, ledger must still go.
#[tokio::test]
async fn review_r2_uninstall_when_file_already_gone_still_cleans_up() {
    let user = Arc::new(FakeUserApps::default());
    let desktop = Arc::new(FakeDesktop::default());
    let ledger = Arc::new(FakeLedger::default());
    ledger.seed(sample_app("lumen", "1", AppSource::AppImage));
    user.fail_trash(InstallError::NotFound);
    let uc = UninstallApp::new(
        Arc::new(FakeSystemPackages::default()),
        user,
        desktop.clone(),
        ledger.clone(),
    );
    let out = uc
        .execute(&AppId::from_name("lumen"), &RecordingObserver::default())
        .await;
    assert_eq!(out, UninstallOutcome::Removed, "stuck: {out:?}");
    assert_eq!(
        desktop.calls(),
        ["remove lumen"],
        "menu entry/icons orphaned"
    );
    assert!(ledger.apps().is_empty());
}

/// Ledger says 1.0, disk has 2.0 (cancel mid-update, manual copy): list must show 2.0.
#[tokio::test]
async fn review_r2_reconcile_refreshes_version_from_disk() {
    let user = Arc::new(FakeUserApps::default());
    let ledger = Arc::new(FakeLedger::default());
    ledger.seed(sample_app("lumen", "1.0", AppSource::AppImage));
    user.set_scan(vec![sample_app("lumen", "2.0", AppSource::AppImage)]);
    let uc = ListInstalledApps::new(user, ledger, Arc::new(FakeSystemPackages::default()));
    let apps = uc.execute().await.unwrap();
    assert_eq!(apps[0].version.as_str(), "2.0");
}

/// Two JsonLedger handles on one file (like two processes) lose updates: lock is in-process.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn review_r2_two_ledger_handles_do_not_lose_updates() {
    let dir = tempfile::tempdir().unwrap();
    let mut tasks = Vec::new();
    for i in 0..40 {
        let l = JsonLedger::new(dir.path());
        tasks.push(tokio::spawn(async move {
            l.record(&sample_app(&format!("app-{i}"), "1", AppSource::AppImage))
                .await
        }));
    }
    for t in tasks {
        let _ = t.await.unwrap();
    }
    let n = JsonLedger::new(dir.path()).list().await.unwrap().len();
    assert_eq!(n, 40, "lost {} updates", 40 - n);
}

/// Hostile names/paths round-trip through GIO's own Exec parser. Expected OK.
#[test]
fn review_r2_exec_roundtrips_through_gio() {
    use gio::prelude::AppInfoExt;
    let dir = tempfile::tempdir().unwrap();
    let names = [
        "a b.AppImage",
        "q\"uote.AppImage",
        "d$ollar`tick.AppImage",
        "back\\slash.AppImage",
        "semi;colon&amp'x.AppImage",
    ];
    let paths: Vec<String> = names
        .iter()
        .map(|n| {
            let p = dir.path().join(n);
            fs::write(&p, b"#!/bin/sh\n").unwrap();
            fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
            p.to_string_lossy().into_owned()
        })
        .collect();
    for (i, p) in paths.iter().enumerate() {
        let text = render_entry("Evil\nExec=/bin/false %", Path::new(p), "icon");
        let f = dir.path().join(format!("t{i}.desktop"));
        fs::write(&f, &text).unwrap();
        let info = gio_unix::DesktopAppInfo::from_filename(&f)
            .unwrap_or_else(|| panic!("gio refuses entry for {p:?}:\n{text}"));
        let cmd = info.commandline().unwrap().to_string_lossy().into_owned();
        let argv = glib::shell_parse_argv(&cmd).unwrap();
        assert_eq!(
            argv[0].to_string_lossy(),
            p.as_str(),
            "cmd={cmd} text={text}"
        );
        assert_eq!(info.name().as_str(), "Evil\nExec=/bin/false %");
    }
}

/// Stale `.partial` from a crash is overwritten and never listed. Expected OK.
#[tokio::test]
async fn review_r2_stale_partial_is_harmless() {
    let src = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join(".lumen-notes.partial"), b"half").unwrap();
    let store = AppImageStore::new(root.path()).with_trash(trash);
    assert!(store.scan().await.unwrap().is_empty());
    let f = mk(src.path(), "x.AppImage", "Lumen Notes", "1.0");
    let p = store.install(&f, &meta_of(&f)).await.unwrap();
    assert_eq!(fs::read(&p).unwrap(), fs::read(&f.path).unwrap());
    assert_eq!(
        fs::metadata(&p).unwrap().permissions().mode() & 0o777,
        0o755
    );
    assert!(!root.path().join(".lumen-notes.partial").exists());
}

/// Hostile display names stay inside the store. Expected OK.
#[tokio::test]
async fn review_r2_hostile_names_stay_in_store() {
    let src = tempfile::tempdir().unwrap();
    let base = tempfile::tempdir().unwrap();
    let root = base.path().join("Applications");
    let store = AppImageStore::new(&root).with_trash(trash);
    let names = [
        "../../evil",
        "..",
        "a/../../b",
        ".hidden",
        "x\ny",
        "%n \"q\" $(id) `id`",
        "\\..\\x",
    ];
    for (i, n) in names.iter().enumerate() {
        let f = mk(src.path(), &format!("{i}.AppImage"), n, "1.0");
        let p = store.install(&f, &meta_of(&f)).await.unwrap();
        assert_eq!(p.parent().unwrap(), root, "{n:?} -> {p:?}");
    }
    assert_eq!(fs::read_dir(base.path()).unwrap().count(), 1);
}

/// A '%' in the file name (file_safe_name keeps it) makes GIO refuse the entry: app cannot launch.
#[test]
fn review_r2_percent_in_file_name_is_launchable() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("100%.AppImage");
    fs::write(&p, b"#!/bin/sh\n").unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    let f = dir.path().join("t.desktop");
    fs::write(&f, render_entry("X", &p, "icon")).unwrap();
    assert!(
        gio_unix::DesktopAppInfo::from_filename(&f).is_some(),
        "GIO rejects Exec with %% in path"
    );
}
