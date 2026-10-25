//! Shared app state, managed by Tauri. Built only by `composition::build_state`.

use std::path::PathBuf;
use std::sync::Mutex;

use crate::composition::{
    CancelInstallUc, DefaultStatusUc, DismissPromptUc, GetAppearanceUc, InstallPackageUc,
    LaunchAppUc, ListInstalledUc, MakeDefaultUc, OpenPackageUc, UninstallAppUc, WatchAppearanceUc,
};

pub struct AppState {
    pub launch_file: LaunchFile,
    pub open_package: OpenPackageUc,
    pub install_package: InstallPackageUc,
    pub cancel_install: CancelInstallUc,
    pub list_installed: ListInstalledUc,
    pub uninstall_app: UninstallAppUc,
    pub launch_app: LaunchAppUc,
    pub default_status: DefaultStatusUc,
    pub make_default: MakeDefaultUc,
    pub dismiss_prompt: DismissPromptUc,
    pub get_appearance: GetAppearanceUc,
    pub watch_appearance: WatchAppearanceUc,
}

/// The file from the first launch's argv, handed out once.
#[derive(Default)]
pub struct LaunchFile(Mutex<Option<PathBuf>>);

impl LaunchFile {
    pub fn new(path: Option<PathBuf>) -> Self {
        Self(Mutex::new(path))
    }

    /// Returns the path once. Later calls give `None`.
    pub fn take(&self) -> Option<PathBuf> {
        // A poisoned lock only means another thread panicked; the data is still valid.
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_file_is_taken_once() {
        let f = LaunchFile::new(Some(PathBuf::from("/tmp/a.rpm")));
        assert_eq!(f.take(), Some(PathBuf::from("/tmp/a.rpm")));
        assert_eq!(f.take(), None);
    }
}
