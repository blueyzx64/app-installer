//! `AppLedger`: our record of installed apps, a JSON file.
//!
//! Path: `$XDG_DATA_HOME/io.github.blueyzx64.Installer/ledger.json`.
//! Schema `{ "version": 1, "apps": [...] }`. Icons are not stored in the file (they would
//! bloat it); on `list` they are read back from the hicolor files we wrote.
//! Writes are atomic (temp file, fsync, rename). A file that is not valid JSON is moved
//! aside to `ledger.json.corrupt` and treated as empty, so a damaged ledger never blocks
//! the library. A file from a newer schema is left alone and reported as an error.

use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use installer_application::ports::AppLedger;
use installer_domain::{AppId, AppSource, BackendError, InstalledApp, Version};
use serde::{Deserialize, Serialize};

use crate::desktop_entries::{APP_PREFIX, DesktopEntries, write_atomic};

pub const LEDGER_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
struct LedgerFile {
    version: u32,
    #[serde(default)]
    apps: Vec<Record>,
}

/// An `InstalledApp` without the icon bytes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    id: AppId,
    name: String,
    display_name: String,
    version: String,
    source: AppSource,
    installed_at: i64,
    desktop_id: Option<String>,
    path: Option<PathBuf>,
}

impl From<&InstalledApp> for Record {
    fn from(a: &InstalledApp) -> Self {
        Record {
            id: a.id.clone(),
            name: a.name.clone(),
            display_name: a.display_name.clone(),
            version: a.version.as_str().to_owned(),
            source: a.source,
            installed_at: a.installed_at,
            desktop_id: a.desktop_id.clone(),
            path: a.path.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct JsonLedger {
    file: PathBuf,
    icons: DesktopEntries,
    lock: Arc<Mutex<()>>,
}

impl JsonLedger {
    /// `data_home` is `$XDG_DATA_HOME` (injected for tests).
    pub fn new(data_home: impl Into<PathBuf>) -> Self {
        let data_home = data_home.into();
        Self {
            file: data_home.join(APP_PREFIX).join("ledger.json"),
            icons: DesktopEntries::new(&data_home),
            lock: Arc::default(),
        }
    }

    pub fn from_env() -> Option<Self> {
        Some(Self::new(crate::desktop_entries::default_data_home()?))
    }

    fn read(&self) -> Result<Vec<Record>, BackendError> {
        let text = match fs::read_to_string(&self.file) {
            Ok(t) => t,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(io_backend(e)),
        };
        match serde_json::from_str::<LedgerFile>(&text) {
            Ok(f) if f.version > LEDGER_VERSION => Err(BackendError::Failed(format!(
                "ledger is from a newer version ({})",
                f.version
            ))),
            Ok(f) => Ok(f.apps),
            Err(e) => {
                tracing::warn!(error = %e, "ledger is damaged; starting empty");
                let mut aside = self.file.clone().into_os_string();
                aside.push(".corrupt");
                let _ = fs::rename(&self.file, PathBuf::from(aside));
                Ok(Vec::new())
            }
        }
    }

    fn write(&self, apps: Vec<Record>) -> Result<(), BackendError> {
        let file = LedgerFile {
            version: LEDGER_VERSION,
            apps,
        };
        let json =
            serde_json::to_vec_pretty(&file).map_err(|e| BackendError::Failed(e.to_string()))?;
        write_atomic(&self.file, &json, 0o600).map_err(io_backend)
    }

    /// Run `f` holding the in-process mutex and an exclusive `flock` on `ledger.lock`, so
    /// several handles or app instances never lose each other's updates. (The data file is
    /// replaced by rename, so the lock lives on a separate file.)
    fn locked<T>(&self, f: impl FnOnce() -> Result<T, BackendError>) -> Result<T, BackendError> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| BackendError::Failed("ledger lock poisoned".into()))?;
        let dir = self
            .file
            .parent()
            .ok_or_else(|| BackendError::Failed("ledger has no folder".into()))?;
        fs::create_dir_all(dir).map_err(io_backend)?;
        let lock_file = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(dir.join("ledger.lock"))
            .map_err(io_backend)?;
        lock_file.lock().map_err(io_backend)?;
        let result = f();
        let _ = lock_file.unlock();
        result
    }

    fn update(&self, f: impl FnOnce(&mut Vec<Record>)) -> Result<(), BackendError> {
        self.locked(|| {
            let mut apps = self.read()?;
            f(&mut apps);
            self.write(apps)
        })
    }

    fn list_blocking(&self) -> Result<Vec<InstalledApp>, BackendError> {
        let records = self.locked(|| self.read())?;
        Ok(records
            .into_iter()
            .map(|r| InstalledApp {
                icon: self.icons.load_icon(r.id.as_str()),
                id: r.id,
                name: r.name,
                display_name: r.display_name,
                version: Version::new(r.version),
                source: r.source,
                installed_at: r.installed_at,
                desktop_id: r.desktop_id,
                path: r.path,
            })
            .collect())
    }
}

/// I/O failures carry only the error kind, never the OS message.
fn io_backend(e: io::Error) -> BackendError {
    BackendError::Io(format!("io error: {:?}", e.kind()))
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, BackendError> + Send + 'static,
) -> Result<T, BackendError> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| BackendError::Failed(e.to_string()))?
}

impl AppLedger for JsonLedger {
    async fn record(&self, app: &InstalledApp) -> Result<(), BackendError> {
        let (this, rec) = (self.clone(), Record::from(app));
        blocking(move || {
            this.update(|apps| match apps.iter_mut().find(|r| r.id == rec.id) {
                Some(slot) => *slot = rec,
                None => apps.push(rec),
            })
        })
        .await
    }

    async fn forget(&self, id: &AppId) -> Result<(), BackendError> {
        let (this, id) = (self.clone(), id.clone());
        blocking(move || this.update(|apps| apps.retain(|r| r.id != id))).await
    }

    async fn list(&self) -> Result<Vec<InstalledApp>, BackendError> {
        let this = self.clone();
        blocking(move || this.list_blocking()).await
    }
}

#[cfg(test)]
mod tests {
    use installer_domain::{IconData, IconKind};

    use super::*;

    fn app(name: &str, version: &str) -> InstalledApp {
        InstalledApp {
            id: AppId::from_name(name),
            name: AppId::from_name(name).as_str().to_owned(),
            display_name: name.into(),
            version: Version::new(version),
            icon: None,
            source: AppSource::AppImage,
            installed_at: 1_700_000_000,
            desktop_id: Some("x.desktop".into()),
            path: Some("/home/u/Applications/x.AppImage".into()),
        }
    }

    #[tokio::test]
    async fn record_list_forget_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let l = JsonLedger::new(dir.path());
        assert!(l.list().await.unwrap().is_empty());
        l.record(&app("Lumen Notes", "1.0")).await.unwrap();
        l.record(&app("Other", "2.0")).await.unwrap();
        // Same id replaces.
        l.record(&app("Lumen Notes", "1.1")).await.unwrap();
        let got = l.list().await.unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].version.as_str(), "1.1");
        assert_eq!(got[0], app("Lumen Notes", "1.1"));
        l.forget(&AppId::from_name("Lumen Notes")).await.unwrap();
        l.forget(&AppId::from_name("never there")).await.unwrap();
        let got = l.list().await.unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].id.as_str(), "other");
    }

    #[tokio::test]
    async fn file_is_versioned_json_at_expected_path() {
        let dir = tempfile::tempdir().unwrap();
        let l = JsonLedger::new(dir.path());
        l.record(&app("A", "1")).await.unwrap();
        let path = dir.path().join("io.github.blueyzx64.Installer/ledger.json");
        let v: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert_eq!(v["version"], 1);
        assert_eq!(v["apps"][0]["displayName"], "A");
        assert!(v["apps"][0].get("icon").is_none());
    }

    #[tokio::test]
    async fn icon_is_loaded_from_hicolor() {
        let dir = tempfile::tempdir().unwrap();
        let l = JsonLedger::new(dir.path());
        let a = app("Lumen", "1");
        l.record(&a).await.unwrap();
        let de = DesktopEntries::new(dir.path());
        let icon = IconData {
            kind: IconKind::Svg,
            bytes: b"<svg/>".to_vec(),
        };
        use installer_application::ports::DesktopIntegration;
        de.install_entry(&a, Some(&icon)).await.unwrap();
        assert_eq!(l.list().await.unwrap()[0].icon, Some(icon));
    }

    #[tokio::test]
    async fn damaged_file_is_moved_aside() {
        let dir = tempfile::tempdir().unwrap();
        let l = JsonLedger::new(dir.path());
        l.record(&app("A", "1")).await.unwrap();
        let path = dir.path().join("io.github.blueyzx64.Installer/ledger.json");
        fs::write(&path, b"{ not json").unwrap();
        assert!(l.list().await.unwrap().is_empty());
        assert!(path.with_extension("json.corrupt").exists());
        l.record(&app("B", "1")).await.unwrap();
        assert_eq!(l.list().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn newer_schema_is_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let l = JsonLedger::new(dir.path());
        let path = dir.path().join("io.github.blueyzx64.Installer/ledger.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, br#"{"version": 99, "apps": []}"#).unwrap();
        assert!(l.list().await.is_err());
        assert!(l.record(&app("A", "1")).await.is_err());
        assert!(fs::read_to_string(&path).unwrap().contains("99"));
    }

    #[tokio::test]
    async fn concurrent_records_do_not_lose_entries() {
        let dir = tempfile::tempdir().unwrap();
        let l = JsonLedger::new(dir.path());
        let mut tasks = Vec::new();
        for i in 0..8 {
            let l = l.clone();
            tasks.push(tokio::spawn(async move {
                l.record(&app(&format!("App {i}"), "1")).await
            }));
        }
        for t in tasks {
            t.await.unwrap().unwrap();
        }
        assert_eq!(l.list().await.unwrap().len(), 8);
    }
}
