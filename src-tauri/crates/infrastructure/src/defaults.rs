//! `DefaultHandlerRegistry` over GIO (F-50). No `xdg-mime` spawn.
//!
//! GIO writes `~/.config/mimeapps.list` for us. The "Not Now" memory lives in
//! `$XDG_CONFIG_HOME/io.github.blueyzx64.Installer/state.json` (atomic write).

use std::path::{Path, PathBuf};

use installer_application::ports::DefaultHandlerRegistry;
use installer_domain::{BackendError, DefaultStatus};
use serde_json::{Map, Value};

pub const APP_ID: &str = "io.github.blueyzx64.Installer";
pub const DESKTOP_ID: &str = "io.github.blueyzx64.Installer.desktop";

const BASE_MIMES: [&str; 3] = [
    "application/x-rpm",
    "application/vnd.appimage",
    "application/x-iso9660-appimage",
];
const DEB_MIME: &str = "application/vnd.debian.binary-package";
const ASKED_KEY: &str = "askedBefore";

/// The MIME types we want to own.
pub fn mime_types(include_deb: bool) -> Vec<&'static str> {
    let mut v = BASE_MIMES.to_vec();
    if include_deb {
        v.push(DEB_MIME);
    }
    v
}

/// Small seam over GIO so the logic is testable with a fake.
pub trait MimeDefaults: Send + Sync {
    /// Desktop id of the current default handler, if any.
    fn default_for(&self, mime: &str) -> Option<String>;
    fn set_default(&self, desktop_id: &str, mime: &str) -> Result<(), BackendError>;
}

/// Real GIO implementation.
#[derive(Debug, Default, Clone, Copy)]
pub struct GioMimeDefaults;

impl MimeDefaults for GioMimeDefaults {
    fn default_for(&self, mime: &str) -> Option<String> {
        use gio::prelude::AppInfoExt;
        gio::AppInfo::default_for_type(mime, false).map(|a| a.id().map(|i| i.to_string()))?
    }

    fn set_default(&self, desktop_id: &str, mime: &str) -> Result<(), BackendError> {
        use gio::prelude::AppInfoExt;
        let app = gio::DesktopAppInfo::new(desktop_id).ok_or_else(|| {
            BackendError::Unavailable(format!("desktop entry {desktop_id} not found"))
        })?;
        app.set_as_default_for_type(mime)
            .map_err(|e| BackendError::Failed(format!("set default for {mime}: {e}")))
    }
}

/// Default location of `state.json`.
pub fn default_state_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"));
    base.join(APP_ID).join("state.json")
}

pub struct GioDefaultHandlerRegistry<M = GioMimeDefaults> {
    mime: M,
    state_path: PathBuf,
    desktop_id: String,
    include_deb: bool,
}

impl GioDefaultHandlerRegistry<GioMimeDefaults> {
    pub fn new() -> Self {
        Self::with_parts(GioMimeDefaults, default_state_path(), false)
    }
}

impl Default for GioDefaultHandlerRegistry<GioMimeDefaults> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M: MimeDefaults> GioDefaultHandlerRegistry<M> {
    pub fn with_parts(mime: M, state_path: PathBuf, include_deb: bool) -> Self {
        Self {
            mime,
            state_path,
            desktop_id: DESKTOP_ID.to_string(),
            include_deb,
        }
    }

    /// Also own `.deb` (later feature flag).
    pub fn include_deb(mut self, on: bool) -> Self {
        self.include_deb = on;
        self
    }

    fn is_default(&self) -> bool {
        mime_types(self.include_deb)
            .iter()
            .all(|m| self.mime.default_for(m).as_deref() == Some(self.desktop_id.as_str()))
    }
}

fn read_state(path: &Path) -> Result<Map<String, Value>, BackendError> {
    match std::fs::read_to_string(path) {
        Ok(text) => match serde_json::from_str::<Value>(&text) {
            Ok(Value::Object(m)) => Ok(m),
            // Corrupt or odd file: treat as empty, next write replaces it.
            _ => Ok(Map::new()),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Map::new()),
        Err(e) => Err(BackendError::Io(e.to_string())),
    }
}

fn write_state_atomic(path: &Path, state: &Map<String, Value>) -> Result<(), BackendError> {
    let io = |e: std::io::Error| BackendError::Io(e.to_string());
    let dir = path
        .parent()
        .ok_or_else(|| BackendError::Io("state path has no parent".into()))?;
    std::fs::create_dir_all(dir).map_err(io)?;
    let tmp = path.with_extension("json.tmp");
    let body = serde_json::to_vec_pretty(state).map_err(|e| BackendError::Io(e.to_string()))?;
    {
        use std::io::Write;
        let mut f = std::fs::File::create(&tmp).map_err(io)?;
        f.write_all(&body).map_err(io)?;
        f.sync_all().map_err(io)?;
    }
    std::fs::rename(&tmp, path).map_err(io)
}

impl<M: MimeDefaults> DefaultHandlerRegistry for GioDefaultHandlerRegistry<M> {
    async fn status(&self) -> Result<DefaultStatus, BackendError> {
        let asked_before = read_state(&self.state_path)?
            .get(ASKED_KEY)
            .and_then(Value::as_bool)
            .unwrap_or(false);
        Ok(DefaultStatus {
            is_default: self.is_default(),
            asked_before,
        })
    }

    async fn make_default(&self) -> Result<(), BackendError> {
        for mime in mime_types(self.include_deb) {
            self.mime.set_default(&self.desktop_id, mime)?;
        }
        Ok(())
    }

    async fn dismiss_prompt(&self) -> Result<(), BackendError> {
        let mut state = read_state(&self.state_path)?;
        state.insert(ASKED_KEY.into(), Value::Bool(true));
        write_state_atomic(&self.state_path, &state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Default)]
    struct FakeMime {
        map: Mutex<HashMap<String, String>>,
        fail_on: Option<&'static str>,
    }

    impl MimeDefaults for FakeMime {
        fn default_for(&self, mime: &str) -> Option<String> {
            self.map.lock().unwrap().get(mime).cloned()
        }
        fn set_default(&self, id: &str, mime: &str) -> Result<(), BackendError> {
            if self.fail_on == Some(mime) {
                return Err(BackendError::Failed("boom".into()));
            }
            self.map.lock().unwrap().insert(mime.into(), id.into());
            Ok(())
        }
    }

    fn reg(mime: FakeMime, deb: bool) -> (GioDefaultHandlerRegistry<FakeMime>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join("state.json");
        (GioDefaultHandlerRegistry::with_parts(mime, path, deb), dir)
    }

    #[tokio::test]
    async fn fresh_is_not_default_not_asked() {
        let (r, _d) = reg(FakeMime::default(), false);
        let s = r.status().await.unwrap();
        assert!(!s.is_default && !s.asked_before);
    }

    #[tokio::test]
    async fn make_default_sets_all_and_status_true() {
        let (r, _d) = reg(FakeMime::default(), false);
        r.make_default().await.unwrap();
        assert!(r.status().await.unwrap().is_default);
        assert_eq!(r.mime.map.lock().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn partial_ownership_is_not_default() {
        let m = FakeMime::default();
        m.set_default(DESKTOP_ID, "application/x-rpm").unwrap();
        m.set_default("other.desktop", "application/vnd.appimage")
            .unwrap();
        m.set_default(DESKTOP_ID, "application/x-iso9660-appimage")
            .unwrap();
        let (r, _d) = reg(m, false);
        assert!(!r.status().await.unwrap().is_default);
    }

    #[tokio::test]
    async fn deb_flag_requires_and_sets_deb() {
        let (r, _d) = reg(FakeMime::default(), false);
        r.make_default().await.unwrap();
        let r2 = GioDefaultHandlerRegistry::with_parts(
            FakeMime {
                map: Mutex::new(r.mime.map.lock().unwrap().clone()),
                fail_on: None,
            },
            r.state_path.clone(),
            true,
        );
        assert!(!r2.status().await.unwrap().is_default);
        r2.make_default().await.unwrap();
        assert!(r2.status().await.unwrap().is_default);
        assert_eq!(r2.mime.map.lock().unwrap().len(), 4);
    }

    #[tokio::test]
    async fn make_default_error_propagates() {
        let (r, _d) = reg(
            FakeMime {
                fail_on: Some("application/vnd.appimage"),
                ..Default::default()
            },
            false,
        );
        assert!(matches!(
            r.make_default().await,
            Err(BackendError::Failed(_))
        ));
        assert!(!r.status().await.unwrap().is_default);
    }

    #[tokio::test]
    async fn dismiss_persists_and_leaves_no_tmp() {
        let (r, d) = reg(FakeMime::default(), false);
        r.dismiss_prompt().await.unwrap();
        assert!(r.status().await.unwrap().asked_before);
        let again = GioDefaultHandlerRegistry::with_parts(
            FakeMime::default(),
            d.path().join("sub").join("state.json"),
            false,
        );
        assert!(again.status().await.unwrap().asked_before);
        assert!(!d.path().join("sub").join("state.json.tmp").exists());
    }

    #[tokio::test]
    async fn dismiss_keeps_other_keys_and_survives_corrupt_file() {
        let (r, _d) = reg(FakeMime::default(), false);
        std::fs::create_dir_all(r.state_path.parent().unwrap()).unwrap();
        std::fs::write(&r.state_path, r#"{"other":1}"#).unwrap();
        r.dismiss_prompt().await.unwrap();
        let m = read_state(&r.state_path).unwrap();
        assert_eq!(m.get("other"), Some(&Value::from(1)));
        std::fs::write(&r.state_path, "not json").unwrap();
        assert!(!r.status().await.unwrap().asked_before);
        r.dismiss_prompt().await.unwrap();
        assert!(r.status().await.unwrap().asked_before);
    }

    // HUMAN: after Make Default, Files opens a .rpm with Installer.
    #[test]
    #[ignore = "needs a session with the installed desktop entry"]
    fn real_gio_roundtrip() {
        let g = GioMimeDefaults;
        g.set_default(DESKTOP_ID, "application/x-rpm").unwrap();
        assert_eq!(
            g.default_for("application/x-rpm").as_deref(),
            Some(DESKTOP_ID)
        );
    }
}
