//! In-memory fakes for every port, plus sample data. Enabled by the `testing` feature.
//!
//! Fakes are scripted through `&self` setters (interior mutability) so they can be
//! shared in `Arc`s with the use case under test. Failure setters are sticky: the
//! failure applies to every later call until replaced.
//!
//! These fakes are test code: they may panic on poisoned locks.
#![allow(clippy::unwrap_used, clippy::missing_panics_doc)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use installer_domain::{
    AppId, AppSource, Appearance, Arch, BackendError, DefaultStatus, DistroFamily, HostSystem,
    IconData, InspectError, InstallError, InstallPhase, InstallPlan, InstallProgress, InstallScope,
    InstalledApp, PackageFile, PackageFormat, PackageMetadata, Version,
};

use crate::CancelToken;
use crate::ports::{
    AppLedger, AppearanceObserver, AppearanceSource, Clock, DefaultHandlerRegistry,
    DesktopIntegration, FileHead, FileProbe, HostProbe, InstallObserver, Launcher,
    PackageInspector, SystemPackages, UserApps,
};

// ---- sample data -----------------------------------------------------------------

pub const RPM_HEAD: &[u8] = &[0xED, 0xAB, 0xEE, 0xDB, 3, 0, 0, 0];
pub const DEB_HEAD: &[u8] =
    b"!<arch>\ndebian-binary   1700000000  0     0     100644  4         `\n2.0\n";
pub const APPIMAGE_HEAD: &[u8] = b"\x7fELF\x02\x01\x01\x00AI\x02\x00\x00\x00\x00\x00";

/// "Lumen Notes" 2.0 for x86_64.
pub fn sample_meta(format: PackageFormat) -> PackageMetadata {
    PackageMetadata {
        format,
        name: "lumen-notes".into(),
        display_name: "Lumen Notes".into(),
        version: Version::new("2.0"),
        arch: Arch::X86_64,
        summary: Some("Notes that glow".into()),
        icon: None,
        scope: InstallScope::for_format(format),
        needs_fuse2: false,
    }
}

pub fn sample_file(format: PackageFormat) -> PackageFile {
    let ext = match format {
        PackageFormat::Rpm => "rpm",
        PackageFormat::Deb => "deb",
        PackageFormat::AppImage => "AppImage",
    };
    PackageFile {
        path: PathBuf::from(format!("/home/u/Downloads/lumen-notes.{ext}")),
        format,
        size_bytes: 1000,
    }
}

/// An installed app whose id, name and display name are all `id`.
pub fn sample_app(id: &str, version: &str, source: AppSource) -> InstalledApp {
    InstalledApp {
        id: AppId::from_name(id),
        name: id.to_owned(),
        display_name: id.to_owned(),
        version: Version::new(version),
        icon: None,
        source,
        installed_at: 1,
        desktop_id: Some(format!("{id}.desktop")),
        path: (source == AppSource::AppImage)
            .then(|| PathBuf::from(format!("/home/u/Applications/{id}.AppImage"))),
    }
}

// ---- helpers ---------------------------------------------------------------------

/// A sticky, optional failure.
#[derive(Debug)]
struct Failure<E>(Mutex<Option<E>>);

impl<E> Default for Failure<E> {
    fn default() -> Self {
        Self(Mutex::new(None))
    }
}

impl<E: Clone> Failure<E> {
    fn set(&self, e: E) {
        *self.0.lock().unwrap() = Some(e);
    }
    fn check(&self) -> Result<(), E> {
        match self.0.lock().unwrap().clone() {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}

// ---- observers -------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct RecordingObserver {
    progress: Mutex<Vec<InstallProgress>>,
    plans: Mutex<Vec<InstallPlan>>,
}

impl RecordingObserver {
    pub fn progress(&self) -> Vec<InstallProgress> {
        self.progress.lock().unwrap().clone()
    }
    pub fn phases(&self) -> Vec<InstallPhase> {
        self.progress().into_iter().map(|p| p.phase).collect()
    }
    pub fn plans(&self) -> Vec<InstallPlan> {
        self.plans.lock().unwrap().clone()
    }
}

impl InstallObserver for RecordingObserver {
    fn on_progress(&self, progress: InstallProgress) {
        self.progress.lock().unwrap().push(progress);
    }
    fn on_plan(&self, plan: &InstallPlan) {
        self.plans.lock().unwrap().push(*plan);
    }
}

#[derive(Debug, Default)]
pub struct RecordingAppearanceObserver(Mutex<Vec<Appearance>>);

impl RecordingAppearanceObserver {
    pub fn seen(&self) -> Vec<Appearance> {
        self.0.lock().unwrap().clone()
    }
}

impl AppearanceObserver for RecordingAppearanceObserver {
    fn on_change(&self, appearance: Appearance) {
        self.0.lock().unwrap().push(appearance);
    }
}

// ---- files and inspection --------------------------------------------------------

#[derive(Debug, Default)]
pub struct FakeFileProbe {
    files: Mutex<HashMap<PathBuf, (Vec<u8>, u64)>>,
}

impl FakeFileProbe {
    /// Make `path` readable with these leading bytes and total size.
    pub fn add(&self, path: &Path, head: &[u8], size_bytes: u64) {
        self.files
            .lock()
            .unwrap()
            .insert(path.to_path_buf(), (head.to_vec(), size_bytes));
    }
}

impl FileProbe for FakeFileProbe {
    async fn head(&self, path: &Path, len: usize) -> Result<FileHead, InspectError> {
        let files = self.files.lock().unwrap();
        let (bytes, size) = files.get(path).ok_or(InspectError::NotFound)?;
        Ok(FileHead {
            bytes: bytes.iter().copied().take(len).collect(),
            size_bytes: *size,
        })
    }
}

#[derive(Debug, Default)]
pub struct FakeInspector {
    meta: Mutex<Option<PackageMetadata>>,
    err: Failure<InspectError>,
    formats: Mutex<Option<Vec<PackageFormat>>>,
}

impl FakeInspector {
    pub fn set_meta(&self, meta: PackageMetadata) {
        *self.meta.lock().unwrap() = Some(meta);
    }
    pub fn fail(&self, e: InspectError) {
        self.err.set(e);
    }
    /// Restrict `supports`. By default every format is supported.
    pub fn only_formats(&self, formats: &[PackageFormat]) {
        *self.formats.lock().unwrap() = Some(formats.to_vec());
    }
}

impl PackageInspector for FakeInspector {
    fn supports(&self, format: PackageFormat) -> bool {
        self.formats
            .lock()
            .unwrap()
            .as_ref()
            .is_none_or(|f| f.contains(&format))
    }
    async fn inspect(&self, _file: &PackageFile) -> Result<PackageMetadata, InspectError> {
        self.err.check()?;
        self.meta
            .lock()
            .unwrap()
            .clone()
            .ok_or(InspectError::NotFound)
    }
}

// ---- host and clock --------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct FakeHost(pub HostSystem);

impl FakeHost {
    pub fn fedora() -> Self {
        Self(HostSystem {
            family: DistroFamily::RpmBased,
            distro_name: "Fedora Linux".into(),
            version_id: "45".into(),
            arch: Arch::X86_64,
            has_fuse2: true,
        })
    }
}

impl HostProbe for FakeHost {
    fn host(&self) -> HostSystem {
        self.0.clone()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FakeClock(pub i64);

impl Clock for FakeClock {
    fn now_unix(&self) -> i64 {
        self.0
    }
}

// ---- packages --------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct FakeSystemPackages {
    installed: Mutex<HashMap<String, Version>>,
    plan: Mutex<InstallPlan>,
    plan_err: Failure<BackendError>,
    removal: Mutex<u32>,
    removal_err: Failure<BackendError>,
    installed_err: Failure<BackendError>,
    install_err: Failure<InstallError>,
    remove_err: Failure<InstallError>,
    block: AtomicBool,
    calls: Mutex<Vec<String>>,
}

impl FakeSystemPackages {
    pub fn set_installed(&self, name: &str, version: &str) {
        self.installed
            .lock()
            .unwrap()
            .insert(name.to_owned(), Version::new(version));
    }
    pub fn set_plan(&self, plan: InstallPlan) {
        *self.plan.lock().unwrap() = plan;
    }
    /// Extra packages `removal_plan` reports.
    pub fn set_removal_plan(&self, also_removes: u32) {
        *self.removal.lock().unwrap() = also_removes;
    }
    pub fn fail_removal_plan(&self, e: BackendError) {
        self.removal_err.set(e);
    }
    pub fn fail_installed_version(&self, e: BackendError) {
        self.installed_err.set(e);
    }
    pub fn fail_plan(&self, e: BackendError) {
        self.plan_err.set(e);
    }
    pub fn fail_install(&self, e: InstallError) {
        self.install_err.set(e);
    }
    pub fn fail_remove(&self, e: InstallError) {
        self.remove_err.set(e);
    }
    /// `install` waits for the cancel token, then returns `Cancelled`.
    pub fn block_until_cancelled(&self) {
        self.block.store(true, Ordering::SeqCst);
    }
    /// `plan <path>`, `install <path>`, `remove <name>` in call order.
    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
    fn log(&self, line: String) {
        self.calls.lock().unwrap().push(line);
    }
}

impl SystemPackages for FakeSystemPackages {
    async fn installed_version(&self, name: &str) -> Result<Option<Version>, BackendError> {
        self.installed_err.check()?;
        Ok(self.installed.lock().unwrap().get(name).cloned())
    }

    async fn plan(&self, file: &PackageFile) -> Result<InstallPlan, BackendError> {
        self.log(format!("plan {}", file.path.display()));
        self.plan_err.check()?;
        Ok(*self.plan.lock().unwrap())
    }

    async fn install(
        &self,
        file: &PackageFile,
        observer: &dyn InstallObserver,
        cancel: CancelToken,
    ) -> Result<(), InstallError> {
        self.log(format!("install {}", file.path.display()));
        observer.on_progress(InstallProgress::phase(InstallPhase::WaitingForPermission));
        if self.block.load(Ordering::SeqCst) {
            cancel.cancelled().await;
            return Err(InstallError::Cancelled);
        }
        self.install_err.check()?;
        observer.on_progress(InstallProgress::with_percent(
            InstallPhase::Downloading,
            20,
            None,
        ));
        observer.on_progress(InstallProgress::with_percent(
            InstallPhase::Installing,
            80,
            Some(3),
        ));
        Ok(())
    }

    async fn removal_plan(&self, name: &str) -> Result<u32, BackendError> {
        self.log(format!("removal_plan {name}"));
        self.removal_err.check()?;
        Ok(*self.removal.lock().unwrap())
    }

    async fn remove(&self, name: &str, observer: &dyn InstallObserver) -> Result<(), InstallError> {
        self.log(format!("remove {name}"));
        self.remove_err.check()?;
        observer.on_progress(InstallProgress::phase(InstallPhase::Installing));
        self.installed.lock().unwrap().remove(name);
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct FakeUserApps {
    scan: Mutex<Vec<InstalledApp>>,
    trashed: Mutex<Vec<AppId>>,
    install_err: Failure<InstallError>,
    trash_err: Failure<InstallError>,
    scan_err: Failure<InstallError>,
}

impl FakeUserApps {
    pub fn set_scan(&self, apps: Vec<InstalledApp>) {
        *self.scan.lock().unwrap() = apps;
    }
    pub fn trashed(&self) -> Vec<AppId> {
        self.trashed.lock().unwrap().clone()
    }
    pub fn fail_install(&self, e: InstallError) {
        self.install_err.set(e);
    }
    pub fn fail_trash(&self, e: InstallError) {
        self.trash_err.set(e);
    }
    pub fn fail_scan(&self, e: InstallError) {
        self.scan_err.set(e);
    }
}

impl UserApps for FakeUserApps {
    async fn install(
        &self,
        _file: &PackageFile,
        meta: &PackageMetadata,
    ) -> Result<PathBuf, InstallError> {
        self.install_err.check()?;
        Ok(PathBuf::from(format!(
            "/home/u/Applications/{}.AppImage",
            meta.display_name
        )))
    }

    async fn trash(&self, app: &InstalledApp) -> Result<(), InstallError> {
        self.trash_err.check()?;
        self.trashed.lock().unwrap().push(app.id.clone());
        Ok(())
    }

    async fn scan(&self) -> Result<Vec<InstalledApp>, InstallError> {
        self.scan_err.check()?;
        Ok(self.scan.lock().unwrap().clone())
    }
}

// ---- desktop ---------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct FakeDesktop {
    install_err: Failure<InstallError>,
    remove_err: Failure<InstallError>,
    calls: Mutex<Vec<String>>,
    missing: Mutex<Vec<AppId>>,
}

impl FakeDesktop {
    /// Pretend the entry of `id` is gone (until the next `install_entry` for it).
    pub fn set_missing(&self, id: &AppId) {
        self.missing.lock().unwrap().push(id.clone());
    }
    pub fn fail_install(&self, e: InstallError) {
        self.install_err.set(e);
    }
    pub fn fail_remove(&self, e: InstallError) {
        self.remove_err.set(e);
    }
    /// `install <id>` / `remove <id>` in call order.
    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

impl DesktopIntegration for FakeDesktop {
    async fn install_entry(
        &self,
        app: &InstalledApp,
        _icon: Option<&IconData>,
    ) -> Result<String, InstallError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("install {}", app.id));
        self.install_err.check()?;
        self.missing.lock().unwrap().retain(|m| m != &app.id);
        Ok(format!("io.installer.{}.desktop", app.id))
    }

    async fn entry_intact(&self, app: &InstalledApp, _with_icon: bool) -> bool {
        !self.missing.lock().unwrap().contains(&app.id)
    }

    async fn remove_entry(&self, app: &InstalledApp) -> Result<(), InstallError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("remove {}", app.id));
        self.remove_err.check()?;
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct FakeLauncher {
    launched: Mutex<Vec<String>>,
    err: Failure<BackendError>,
}

impl FakeLauncher {
    pub fn launched(&self) -> Vec<String> {
        self.launched.lock().unwrap().clone()
    }
    pub fn fail(&self, e: BackendError) {
        self.err.set(e);
    }
}

impl Launcher for FakeLauncher {
    async fn launch(&self, app: &InstalledApp) -> Result<(), BackendError> {
        self.err.check()?;
        self.launched.lock().unwrap().push(app.id.to_string());
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct FakeDefaults {
    state: Mutex<Option<DefaultStatus>>,
    err: Failure<BackendError>,
}

impl FakeDefaults {
    pub fn fail(&self, e: BackendError) {
        self.err.set(e);
    }
    fn update(&self, f: impl FnOnce(&mut DefaultStatus)) -> Result<(), BackendError> {
        self.err.check()?;
        let mut g = self.state.lock().unwrap();
        f(g.get_or_insert(DefaultStatus {
            is_default: false,
            asked_before: false,
        }));
        Ok(())
    }
}

impl DefaultHandlerRegistry for FakeDefaults {
    async fn status(&self) -> Result<DefaultStatus, BackendError> {
        self.update(|_| {})?;
        Ok((*self.state.lock().unwrap()).unwrap())
    }
    async fn make_default(&self) -> Result<(), BackendError> {
        self.update(|s| {
            s.is_default = true;
            s.asked_before = true;
        })
    }
    async fn dismiss_prompt(&self) -> Result<(), BackendError> {
        self.update(|s| s.asked_before = true)
    }
}

// ---- ledger and appearance -------------------------------------------------------

#[derive(Debug, Default)]
pub struct FakeLedger {
    apps: Mutex<Vec<InstalledApp>>,
    err: Failure<BackendError>,
}

impl FakeLedger {
    /// Add an entry without going through the port (no failure check).
    pub fn seed(&self, app: InstalledApp) {
        self.apps.lock().unwrap().push(app);
    }
    pub fn apps(&self) -> Vec<InstalledApp> {
        self.apps.lock().unwrap().clone()
    }
    /// Make every port call fail.
    pub fn fail(&self, e: BackendError) {
        self.err.set(e);
    }
}

impl AppLedger for FakeLedger {
    async fn record(&self, app: &InstalledApp) -> Result<(), BackendError> {
        self.err.check()?;
        let mut apps = self.apps.lock().unwrap();
        apps.retain(|a| a.id != app.id);
        apps.push(app.clone());
        Ok(())
    }
    async fn forget(&self, id: &AppId) -> Result<(), BackendError> {
        self.err.check()?;
        self.apps.lock().unwrap().retain(|a| &a.id != id);
        Ok(())
    }
    async fn list(&self) -> Result<Vec<InstalledApp>, BackendError> {
        self.err.check()?;
        Ok(self.apps.lock().unwrap().clone())
    }
}

#[derive(Default)]
pub struct FakeAppearance {
    current: Mutex<Appearance>,
    err: Failure<BackendError>,
    observers: Mutex<Vec<Arc<dyn AppearanceObserver>>>,
}

impl FakeAppearance {
    pub fn set(&self, a: Appearance) {
        *self.current.lock().unwrap() = a;
    }
    pub fn fail(&self, e: BackendError) {
        self.err.set(e);
    }
    /// Change the appearance and notify every watcher.
    pub fn emit(&self, a: Appearance) {
        self.set(a.clone());
        for o in self.observers.lock().unwrap().iter() {
            o.on_change(a.clone());
        }
    }
}

impl AppearanceSource for FakeAppearance {
    async fn snapshot(&self) -> Result<Appearance, BackendError> {
        self.err.check()?;
        Ok(self.current.lock().unwrap().clone())
    }
    fn watch(&self, observer: Arc<dyn AppearanceObserver>) {
        self.observers.lock().unwrap().push(observer);
    }
}
