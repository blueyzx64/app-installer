//! Composition root: the only place that names concrete adapter types.
//!
//! Every port is wired to its real adapter. The application crate's `testing`
//! fakes are used only in tests (dev-dependency). A shell-side swap is one
//! `type` alias plus one constructor line.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use installer_application::PreviewRegistry;
use installer_application::ports::Clock;
use installer_application::use_cases::{
    CancelInstall, DismissDefaultPrompt, GetAppearance, GetDefaultHandlerStatus, InstallPackage,
    LaunchApp, ListInstalledApps, MakeDefaultHandler, OpenPackage, UninstallApp, UninstallPlan,
    WatchAppearance,
};
use installer_infrastructure::appearance::GnomeAppearanceSource;
use installer_infrastructure::appimage_store::AppImageStore;
use installer_infrastructure::defaults::GioDefaultHandlerRegistry;
use installer_infrastructure::desktop_entries::{DesktopEntries, default_data_home};
use installer_infrastructure::file_probe::FsFileProbe;
use installer_infrastructure::host::OsReleaseProbe;
use installer_infrastructure::inspect::appimage::AppImageInspector;
use installer_infrastructure::inspect::deb::DebInspector;
use installer_infrastructure::inspect::rpm::RpmInspector;
use installer_infrastructure::launcher::GioLauncher;
use installer_infrastructure::ledger::JsonLedger;

use crate::fallbacks::{AnySystemPackages, InspectorChain, NoInspector};
use crate::state::{AppState, LaunchFile};

// Port implementations in use. Swap these when an adapter changes.
type Probe = FsFileProbe;
// One inspector per format, chained: AppImage, then RPM, then DEB.
type Inspector = InspectorChain<
    AppImageInspector,
    InspectorChain<RpmInspector, InspectorChain<DebInspector, NoInspector>>,
>;
type Host = OsReleaseProbe;
// PackageKit, else dnf5daemon, else unavailable (D-020). Chosen at startup.
type System = AnySystemPackages;
type User = AppImageStore;
type Desktop = DesktopEntries;
type Ledger = JsonLedger;
type Launch = GioLauncher;
type Defaults = GioDefaultHandlerRegistry;
type Appearance = GnomeAppearanceSource;

pub type OpenPackageUc = OpenPackage<Probe, Inspector, Host, System, Ledger>;
pub type InstallPackageUc = InstallPackage<System, User, Desktop, Ledger, SystemClock>;
pub type ListInstalledUc = ListInstalledApps<User, Ledger, System, Desktop>;
pub type UninstallAppUc = UninstallApp<System, User, Desktop, Ledger>;
pub type UninstallPlanUc = UninstallPlan<System, Ledger>;
pub type LaunchAppUc = LaunchApp<Ledger, Launch>;
pub type DefaultStatusUc = GetDefaultHandlerStatus<Defaults>;
pub type MakeDefaultUc = MakeDefaultHandler<Defaults>;
pub type DismissPromptUc = DismissDefaultPrompt<Defaults>;
pub type GetAppearanceUc = GetAppearance<Appearance>;
pub type WatchAppearanceUc = WatchAppearance<Appearance>;
pub type CancelInstallUc = CancelInstall;

/// Wall clock from the OS.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_unix(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|d| i64::try_from(d.as_secs()).ok())
            .unwrap_or(0)
    }
}

fn inspector() -> Inspector {
    InspectorChain::new(
        AppImageInspector,
        InspectorChain::new(
            RpmInspector::new(),
            InspectorChain::new(DebInspector::new(), NoInspector),
        ),
    )
}

/// `$XDG_DATA_HOME`, else `~/.local/share`. Without a home the temp dir keeps
/// the app running (nothing survives a reboot, which is the honest outcome).
fn data_home() -> PathBuf {
    default_data_home().unwrap_or_else(|| {
        log::warn!("no home directory; using the temp directory");
        std::env::temp_dir()
    })
}

fn app_store() -> AppImageStore {
    AppImageStore::from_env().unwrap_or_else(|| AppImageStore::new(std::env::temp_dir()))
}

/// Wire every use case to its real adapter.
pub fn build_state(launch_file: Option<PathBuf>) -> AppState {
    let data_home = data_home();
    let probe = Arc::new(FsFileProbe);
    let inspector = Arc::new(inspector());
    let host = Arc::new(OsReleaseProbe::new());
    // One short bus probe at startup; before the event loop, so block here.
    let system = Arc::new(tauri::async_runtime::block_on(AnySystemPackages::detect()));
    let user = Arc::new(app_store());
    let desktop = Arc::new(DesktopEntries::new(&data_home));
    let ledger = Arc::new(JsonLedger::new(&data_home));
    let launcher = Arc::new(GioLauncher::with_applications_dir(
        data_home.join("applications"),
    ));
    let defaults = Arc::new(GioDefaultHandlerRegistry::new());
    // The use cases hold this Arc, so the watcher lives as long as `AppState`.
    let appearance = Arc::new(GnomeAppearanceSource::new());
    let clock = Arc::new(SystemClock);
    let registry = Arc::new(PreviewRegistry::new());

    AppState {
        launch_file: LaunchFile::new(launch_file),
        open_package: OpenPackage::new(
            probe,
            inspector,
            host,
            system.clone(),
            ledger.clone(),
            registry.clone(),
        ),
        install_package: InstallPackage::new(
            system.clone(),
            user.clone(),
            desktop.clone(),
            ledger.clone(),
            clock,
            registry.clone(),
        ),
        cancel_install: CancelInstall::new(registry.clone()),
        list_installed: ListInstalledApps::new(
            user.clone(),
            ledger.clone(),
            system.clone(),
            desktop.clone(),
        ),
        uninstall_plan: UninstallPlan::new(system.clone(), ledger.clone()),
        uninstall_app: UninstallApp::new(system, user, desktop, ledger.clone())
            .with_registry(registry.clone()),
        launch_app: LaunchApp::new(ledger, launcher),
        default_status: GetDefaultHandlerStatus::new(defaults.clone()),
        make_default: MakeDefaultHandler::new(defaults.clone()),
        dismiss_prompt: DismissDefaultPrompt::new(defaults),
        get_appearance: GetAppearance::new(appearance.clone()),
        watch_appearance: WatchAppearance::new(appearance),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_is_after_2024() {
        assert!(SystemClock.now_unix() > 1_700_000_000);
    }

    #[test]
    fn state_builds_and_hands_out_launch_file_once() {
        let state = build_state(Some(PathBuf::from("/tmp/a.rpm")));
        assert_eq!(state.launch_file.take(), Some(PathBuf::from("/tmp/a.rpm")));
        assert_eq!(state.launch_file.take(), None);
    }
}
