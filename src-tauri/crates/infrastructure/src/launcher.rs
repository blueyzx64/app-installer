//! `Launcher` (F-29): start an installed app through GIO, never through a shell.

use std::path::PathBuf;

use gio::prelude::AppInfoExt;
use installer_application::ports::Launcher;
use installer_domain::{BackendError, InstalledApp};

#[derive(Debug, Clone, Default)]
pub struct GioLauncher {
    /// Where our generated `.desktop` files live. Used when GIO's own lookup misses
    /// (for example when `XDG_DATA_HOME` differs from what GIO sees).
    applications_dir: Option<PathBuf>,
}

impl GioLauncher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_applications_dir(dir: impl Into<PathBuf>) -> Self {
        Self {
            applications_dir: Some(dir.into()),
        }
    }

    fn launch_blocking(&self, desktop_id: &str) -> Result<(), BackendError> {
        let info = gio_unix::DesktopAppInfo::new(desktop_id)
            .or_else(|| {
                let dir = self.applications_dir.as_ref()?;
                gio_unix::DesktopAppInfo::from_filename(dir.join(desktop_id))
            })
            .ok_or_else(|| BackendError::Failed(format!("no desktop entry {desktop_id}")))?;
        info.launch(&[], None::<&gio::AppLaunchContext>)
            .map_err(|e| BackendError::Failed(e.to_string()))
    }
}

impl Launcher for GioLauncher {
    async fn launch(&self, app: &InstalledApp) -> Result<(), BackendError> {
        let id = app
            .desktop_id
            .clone()
            .ok_or_else(|| BackendError::Failed("app has no desktop entry".into()))?;
        let this = self.clone();
        tokio::task::spawn_blocking(move || this.launch_blocking(&id))
            .await
            .map_err(|e| BackendError::Failed(e.to_string()))?
    }
}

#[cfg(test)]
mod tests {
    use installer_domain::{AppId, AppSource, Version};

    use super::*;

    fn app(desktop_id: Option<&str>) -> InstalledApp {
        InstalledApp {
            id: AppId::from_name("t"),
            name: "t".into(),
            display_name: "T".into(),
            version: Version::new("1"),
            icon: None,
            source: AppSource::AppImage,
            installed_at: 0,
            desktop_id: desktop_id.map(str::to_owned),
            path: None,
        }
    }

    #[tokio::test]
    async fn no_desktop_id_or_unknown_id_is_an_error() {
        let l = GioLauncher::new();
        assert!(l.launch(&app(None)).await.is_err());
        assert!(l.launch(&app(Some("no.such.app.desktop"))).await.is_err());
    }

    #[tokio::test]
    async fn launches_entry_found_in_applications_dir() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("ran");
        let entry = format!(
            "[Desktop Entry]\nType=Application\nName=T\nExec=/usr/bin/touch {}\n",
            marker.display()
        );
        std::fs::write(dir.path().join("io.test.t.desktop"), entry).unwrap();
        let l = GioLauncher::with_applications_dir(dir.path());
        l.launch(&app(Some("io.test.t.desktop"))).await.unwrap();
        for _ in 0..50 {
            if marker.exists() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        panic!("launched command did not run");
    }
}
