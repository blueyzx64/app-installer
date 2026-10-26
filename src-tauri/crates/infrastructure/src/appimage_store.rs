//! `UserApps` (F-22): the AppImage store in `~/Applications`.
//!
//! Install = copy to `.<slug>.partial`, fsync, chmod 0755, rename to `<Name>.AppImage`.
//! Uninstall = move to the freedesktop Trash through GIO (recoverable).

use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use installer_application::ports::UserApps;
use installer_domain::{
    AppId, AppSource, InstallError, InstalledApp, PackageFile, PackageMetadata, Version,
};

use crate::desktop_entries::io_to_install;
use crate::inspect::appimage::inspect_path;

const COPY_BUF: usize = 1024 * 1024;
const MAX_NAME_CHARS: usize = 100;
const MAX_SUFFIX: u32 = 99;

/// Moves a file to the Trash. Injected so tests do not touch the real Trash.
pub type TrashFn = fn(&Path) -> io::Result<()>;

#[derive(Debug, Clone)]
pub struct AppImageStore {
    root: PathBuf,
    trash: TrashFn,
}

impl AppImageStore {
    /// `root` is `~/Applications`. Trash goes through GIO.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            trash: gio_trash,
        }
    }

    /// `$HOME/Applications`.
    pub fn from_env() -> Option<Self> {
        let home = std::env::var_os("HOME").filter(|h| !h.is_empty())?;
        Some(Self::new(PathBuf::from(home).join("Applications")))
    }

    /// Swap the trash function (tests).
    pub fn with_trash(mut self, trash: TrashFn) -> Self {
        self.trash = trash;
        self
    }

    /// The store root with symlinks resolved (`~/Applications` may be a link, or `/home` may
    /// be). Falls back to the given path while the directory does not exist yet.
    fn canonical_root(&self) -> PathBuf {
        fs::canonicalize(&self.root).unwrap_or_else(|_| self.root.clone())
    }

    fn install_blocking(
        &self,
        file: &PackageFile,
        meta: &PackageMetadata,
    ) -> Result<PathBuf, InstallError> {
        fs::create_dir_all(&self.root).map_err(io_to_install)?;
        let root = self.canonical_root();
        let slug = AppId::from_name(&meta.display_name);

        // Already in the store (for example opened from ~/Applications): make it runnable and
        // make sure the library can see it (`<Name>.AppImage`, not hidden).
        if let Ok(src) = fs::canonicalize(&file.path)
            && src.parent() == Some(root.as_path())
        {
            if is_listed_name(&src) {
                set_executable(&src).map_err(io_to_install)?;
                return Ok(src);
            }
            let target = pick_target(&root, &meta.display_name, &meta.name)?;
            fs::rename(&src, &target).map_err(io_to_install)?;
            set_executable(&target).map_err(io_to_install)?;
            sync_dir(&root).map_err(io_to_install)?;
            return Ok(target);
        }

        let target = pick_target(&root, &meta.display_name, &meta.name)?;
        let partial = root.join(format!(".{}.partial", slug.as_str()));
        let result = copy_synced(&file.path, &partial)
            .and_then(|()| set_executable(&partial))
            .and_then(|()| fs::rename(&partial, &target))
            .and_then(|()| sync_dir(&root));
        if let Err(e) = result {
            let _ = fs::remove_file(&partial);
            return Err(io_to_install(e));
        }
        Ok(target)
    }

    fn scan_blocking(&self) -> Result<Vec<InstalledApp>, InstallError> {
        let root = self.canonical_root();
        // A store we cannot read (missing, a link to an unmounted drive, no access) is an
        // error, never "empty", so the library forgets nothing because of it (D-035, R10).
        let dir = fs::read_dir(&root).map_err(io_to_install)?;
        let mut apps = Vec::new();
        for entry in dir.filter_map(Result::ok) {
            let path = entry.path();
            if !is_listed_name(&path) || !path.is_file() {
                continue;
            }
            let installed_at = fs::metadata(&path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .and_then(|d| i64::try_from(d.as_secs()).ok())
                .unwrap_or(0);
            // The id here is only provisional: the ledger owns ids and matches files by
            // path (D-037). No desktop id either: only an install writes a menu entry.
            let app = match inspect_path(&path) {
                Ok(meta) => InstalledApp {
                    id: AppId::from_name(&meta.display_name),
                    name: meta.name,
                    display_name: meta.display_name,
                    version: meta.version,
                    icon: meta.icon,
                    source: AppSource::AppImage,
                    installed_at,
                    desktop_id: None,
                    path: Some(path),
                },
                // Present but unreadable (damaged, newer format): listed with an empty key
                // and version, so the caller keeps what it knows and records nothing new.
                Err(e) => {
                    tracing::warn!(error = %e, "AppImage in the store cannot be read");
                    let stem = path
                        .file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    InstalledApp {
                        id: AppId::from_name(&stem),
                        name: String::new(),
                        display_name: stem,
                        version: Version::new(""),
                        icon: None,
                        source: AppSource::AppImage,
                        installed_at,
                        desktop_id: None,
                        path: Some(path),
                    }
                }
            };
            apps.push(app);
        }
        apps.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(apps)
    }

    /// Idempotent: a file that is already gone counts as trashed, so the caller still
    /// removes the menu entry and icons.
    fn trash_blocking(&self, app: &InstalledApp) -> Result<(), InstallError> {
        let Some(path) = app.path.as_deref() else {
            return Ok(());
        };
        if !path.exists() && !path.is_symlink() {
            return Ok(());
        }
        // Only ever trash files that live in our store (compare with symlinks resolved).
        let in_store = path
            .parent()
            .and_then(|p| fs::canonicalize(p).ok())
            .is_some_and(|p| p == self.canonical_root());
        if !in_store {
            return Err(InstallError::Unknown("app is not in the app store".into()));
        }
        (self.trash)(path)
            .map_err(|e| InstallError::Unknown(format!("trash failed ({:?})", e.kind())))
    }
}

impl UserApps for AppImageStore {
    async fn install(
        &self,
        file: &PackageFile,
        meta: &PackageMetadata,
    ) -> Result<PathBuf, InstallError> {
        let (this, file, meta) = (self.clone(), file.clone(), meta.clone());
        tokio::task::spawn_blocking(move || this.install_blocking(&file, &meta))
            .await
            .map_err(|e| InstallError::Unknown(e.to_string()))?
    }

    async fn trash(&self, app: &InstalledApp) -> Result<(), InstallError> {
        let (this, app) = (self.clone(), app.clone());
        tokio::task::spawn_blocking(move || this.trash_blocking(&app))
            .await
            .map_err(|e| InstallError::Unknown(e.to_string()))?
    }

    async fn scan(&self) -> Result<Vec<InstalledApp>, InstallError> {
        let this = self.clone();
        tokio::task::spawn_blocking(move || this.scan_blocking())
            .await
            .map_err(|e| InstallError::Unknown(e.to_string()))?
    }
}

/// Real Trash through GIO (`g_file_trash`). No process is spawned.
fn gio_trash(path: &Path) -> io::Result<()> {
    use gio::prelude::FileExt;
    gio::File::for_path(path)
        .trash(None::<&gio::Cancellable>)
        .map_err(io::Error::other)
}

/// Visible `*.AppImage` (any case): what `scan` lists.
fn is_listed_name(path: &Path) -> bool {
    let visible = path
        .file_name()
        .is_some_and(|n| !n.to_string_lossy().starts_with('.'));
    visible
        && path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("appimage"))
}

/// `<Name>.AppImage`; replaces an older copy of the same app; otherwise ` (2)`, ` (3)`...
/// Never picks a stranger's file: when every slot is taken it fails.
fn pick_target(
    root: &Path,
    display_name: &str,
    machine_name: &str,
) -> Result<PathBuf, InstallError> {
    let base = file_safe_name(display_name);
    let mut candidate = root.join(format!("{base}.AppImage"));
    for n in 2..=MAX_SUFFIX + 1 {
        if !candidate.exists() || is_same_app(&candidate, machine_name) {
            return Ok(candidate);
        }
        candidate = root.join(format!("{base} ({n}).AppImage"));
    }
    Err(InstallError::Conflicts)
}

/// A file name that is safe in a directory: no separators, no control chars, no leading dot.
pub fn file_safe_name(display: &str) -> String {
    let cleaned: String = display
        .chars()
        .map(|c| {
            if c == '/' || c == '\\' || c == '\0' || c.is_control() {
                ' '
            } else {
                c
            }
        })
        .collect();
    let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed: String = collapsed
        .trim_start_matches(['.', ' '])
        .chars()
        .take(MAX_NAME_CHARS)
        .collect();
    let trimmed = trimmed.trim().to_owned();
    if trimmed.is_empty() {
        "App".to_owned()
    } else {
        trimmed
    }
}

/// True when `existing` is an AppImage of the same app (same machine name): an update.
fn is_same_app(existing: &Path, machine_name: &str) -> bool {
    inspect_path(existing).is_ok_and(|m| m.name == machine_name)
}

fn copy_synced(from: &Path, to: &Path) -> io::Result<()> {
    let mut src = fs::File::open(from)?;
    let mut dst = fs::File::create(to)?;
    let mut buf = vec![0u8; COPY_BUF];
    loop {
        let n = src.read(&mut buf)?;
        if n == 0 {
            break;
        }
        dst.write_all(&buf[..n])?;
    }
    dst.sync_all()
}

fn set_executable(p: &Path) -> io::Result<()> {
    fs::set_permissions(p, fs::Permissions::from_mode(0o755))
}

fn sync_dir(dir: &Path) -> io::Result<()> {
    fs::File::open(dir)?.sync_all()
}

#[cfg(test)]
mod tests {
    use installer_domain::{Arch, InstallScope, PackageFormat, Version};

    use super::*;
    use crate::inspect::appimage::testkit::{Entry, Spec, build, desktop};

    fn fake_trash(p: &Path) -> io::Result<()> {
        fs::remove_file(p)
    }

    fn make_appimage(dir: &Path, file: &str, name: &str, version: &str) -> PackageFile {
        let bytes = build(Spec {
            entries: vec![Entry::File(
                "/a.desktop",
                desktop(name, "icon", Some(version)),
            )],
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

    fn meta(name: &str) -> PackageMetadata {
        PackageMetadata {
            format: PackageFormat::AppImage,
            name: crate::inspect::appimage::machine_name(name),
            display_name: name.to_owned(),
            version: Version::new("1.0"),
            arch: Arch::X86_64,
            summary: None,
            icon: None,
            scope: InstallScope::User,
            needs_fuse2: false,
        }
    }

    #[tokio::test]
    async fn install_copies_with_mode_and_leaves_no_partial() {
        let src_dir = tempfile::tempdir().unwrap();
        let root_dir = tempfile::tempdir().unwrap();
        let root = root_dir.path().join("Applications"); // does not exist yet
        let store = AppImageStore::new(&root).with_trash(fake_trash);
        let file = make_appimage(src_dir.path(), "dl.AppImage", "Lumen Notes", "1.0");
        let out = store.install(&file, &meta("Lumen Notes")).await.unwrap();
        assert_eq!(out, root.join("Lumen Notes.AppImage"));
        assert_eq!(fs::read(&out).unwrap(), fs::read(&file.path).unwrap());
        assert_eq!(
            fs::metadata(&out).unwrap().permissions().mode() & 0o777,
            0o755
        );
        let names: Vec<_> = fs::read_dir(&root)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["Lumen Notes.AppImage"]);
        assert!(file.path.exists(), "source stays");
    }

    #[tokio::test]
    async fn update_replaces_same_app_but_other_app_gets_suffix() {
        let src_dir = tempfile::tempdir().unwrap();
        let root_dir = tempfile::tempdir().unwrap();
        let store = AppImageStore::new(root_dir.path()).with_trash(fake_trash);
        let v1 = make_appimage(src_dir.path(), "v1.AppImage", "Lumen Notes", "1.0");
        let first = store.install(&v1, &meta("Lumen Notes")).await.unwrap();
        // Same app, new version: replaced in place.
        let v2 = make_appimage(src_dir.path(), "v2.AppImage", "Lumen Notes", "2.0");
        let second = store.install(&v2, &meta("Lumen Notes")).await.unwrap();
        assert_eq!(first, second);
        assert_eq!(fs::read(&second).unwrap(), fs::read(&v2.path).unwrap());
        // A file that is not our app sits at the name: suffix.
        let other_dir = root_dir.path().join("Taken.AppImage");
        fs::write(&other_dir, b"something else").unwrap();
        let t = make_appimage(src_dir.path(), "t.AppImage", "Taken", "1.0");
        let third = store.install(&t, &meta("Taken")).await.unwrap();
        assert_eq!(third, root_dir.path().join("Taken (2).AppImage"));
        assert_eq!(fs::read(&other_dir).unwrap(), b"something else");
    }

    #[tokio::test]
    async fn install_from_inside_store_only_sets_mode() {
        let root_dir = tempfile::tempdir().unwrap();
        let store = AppImageStore::new(root_dir.path()).with_trash(fake_trash);
        let file = make_appimage(root_dir.path(), "Here.AppImage", "Here", "1.0");
        fs::set_permissions(&file.path, fs::Permissions::from_mode(0o644)).unwrap();
        let out = store.install(&file, &meta("Here")).await.unwrap();
        assert_eq!(fs::canonicalize(&file.path).unwrap(), out);
        assert_eq!(
            fs::metadata(&out).unwrap().permissions().mode() & 0o777,
            0o755
        );
    }

    #[tokio::test]
    async fn missing_source_fails_and_cleans_partial() {
        let root_dir = tempfile::tempdir().unwrap();
        let store = AppImageStore::new(root_dir.path()).with_trash(fake_trash);
        let file = PackageFile {
            path: "/nonexistent/x.AppImage".into(),
            format: PackageFormat::AppImage,
            size_bytes: 0,
        };
        assert_eq!(
            store.install(&file, &meta("X")).await,
            Err(InstallError::NotFound)
        );
        assert_eq!(fs::read_dir(root_dir.path()).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn scan_lists_appimages_only() {
        let root_dir = tempfile::tempdir().unwrap();
        make_appimage(root_dir.path(), "B.AppImage", "Beta", "2.1");
        make_appimage(root_dir.path(), "A.appimage", "Alpha", "1.0");
        make_appimage(root_dir.path(), ".hidden.AppImage", "Hidden", "1.0");
        fs::write(root_dir.path().join("notes.txt"), b"x").unwrap();
        fs::write(root_dir.path().join("Junk.AppImage"), b"not an image").unwrap();
        let store = AppImageStore::new(root_dir.path()).with_trash(fake_trash);
        let apps = store.scan().await.unwrap();
        // Sorted by path: A.appimage, B.AppImage, Junk.AppImage (unreadable: empty key).
        let names: Vec<_> = apps.iter().map(|a| a.display_name.as_str()).collect();
        assert_eq!(names, ["Alpha", "Beta", "Junk"]);
        assert_eq!(apps[1].version.as_str(), "2.1");
        assert!(!apps[0].name.is_empty() && apps[2].name.is_empty());
        assert_eq!(apps[2].version.as_str(), "");
        // Scan writes no menu entries, so it claims none.
        assert!(apps.iter().all(|a| a.desktop_id.is_none()));
        let root = fs::canonicalize(root_dir.path()).unwrap();
        assert!(
            apps.iter()
                .all(|a| a.path.as_deref().unwrap().parent() == Some(root.as_path()))
        );
        // Missing store (unmounted drive, deleted folder) is an error, not "no apps".
        let none = AppImageStore::new(root_dir.path().join("nope"));
        assert!(none.scan().await.is_err());
        let dangling = root_dir.path().join("link");
        std::os::unix::fs::symlink(root_dir.path().join("unmounted"), &dangling).unwrap();
        assert!(AppImageStore::new(&dangling).scan().await.is_err());
    }

    #[tokio::test]
    async fn trash_only_inside_store() {
        let root_dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let store = AppImageStore::new(root_dir.path()).with_trash(fake_trash);
        let inside = make_appimage(root_dir.path(), "In.AppImage", "In", "1.0");
        let app = store.scan().await.unwrap().remove(0);
        store.trash(&app).await.unwrap();
        assert!(!inside.path.exists());
        assert_eq!(store.trash(&app).await, Ok(()));

        let out = make_appimage(outside.path(), "Out.AppImage", "Out", "1.0");
        let mut evil = app.clone();
        evil.path = Some(out.path.clone());
        assert!(matches!(
            store.trash(&evil).await,
            Err(InstallError::Unknown(_))
        ));
        assert!(out.path.exists());
    }

    #[test]
    fn file_safe_name_table() {
        let long = "x".repeat(300);
        let long_want = "x".repeat(100);
        let rows = [
            ("Lumen Notes", "Lumen Notes"),
            ("../../etc/passwd", "etc passwd"),
            (".hidden", "hidden"),
            ("  a   b  ", "a b"),
            ("a\nb", "a b"),
            ("", "App"),
            ("...", "App"),
            ("\u{65e5}\u{672c}", "\u{65e5}\u{672c}"),
            (long.as_str(), long_want.as_str()),
        ];
        for (input, want) in rows {
            assert_eq!(file_safe_name(input), want, "{input:?}");
        }
    }
}
