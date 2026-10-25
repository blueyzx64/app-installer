//! Composition root: the only place that names concrete adapter types.
//!
//! Until a real adapter exists for a port, the application crate's `testing`
//! fakes stand in behind clearly named `fake_*` functions (D-012). A real
//! adapter replaces its `fake_*` function and the matching type alias, nothing
//! else.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use installer_application::PreviewRegistry;
use installer_application::ports::Clock;
use installer_application::testing::{
    FakeAppearance, FakeDefaults, FakeDesktop, FakeFileProbe, FakeHost, FakeInspector,
    FakeLauncher, FakeLedger, FakeSystemPackages, FakeUserApps,
};
use installer_application::use_cases::{
    CancelInstall, DismissDefaultPrompt, GetAppearance, GetDefaultHandlerStatus, InstallPackage,
    LaunchApp, ListInstalledApps, MakeDefaultHandler, OpenPackage, UninstallApp, WatchAppearance,
};

use crate::state::{AppState, LaunchFile};

// Port implementations in use. Swap these when a real adapter lands.
type Probe = FakeFileProbe;
type Inspector = FakeInspector;
type Host = FakeHost;
type System = FakeSystemPackages;
type User = FakeUserApps;
type Desktop = FakeDesktop;
type Ledger = FakeLedger;
type Launch = FakeLauncher;
type Defaults = FakeDefaults;
type Appearance = FakeAppearance;

pub type OpenPackageUc = OpenPackage<Probe, Inspector, Host, System, Ledger>;
pub type InstallPackageUc = InstallPackage<System, User, Desktop, Ledger, SystemClock>;
pub type ListInstalledUc = ListInstalledApps<User, Ledger, System>;
pub type UninstallAppUc = UninstallApp<System, User, Desktop, Ledger>;
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

// ---- fakes standing in for adapters that do not exist yet (D-012) ----

fn fake_probe() -> Arc<Probe> {
    Arc::new(FakeFileProbe::default())
}
fn fake_inspector() -> Arc<Inspector> {
    Arc::new(FakeInspector::default())
}
fn fake_host() -> Arc<Host> {
    Arc::new(FakeHost::fedora())
}
fn fake_system_packages() -> Arc<System> {
    Arc::new(FakeSystemPackages::default())
}
fn fake_user_apps() -> Arc<User> {
    Arc::new(FakeUserApps::default())
}
fn fake_desktop() -> Arc<Desktop> {
    Arc::new(FakeDesktop::default())
}
fn fake_ledger() -> Arc<Ledger> {
    Arc::new(FakeLedger::default())
}
fn fake_launcher() -> Arc<Launch> {
    Arc::new(FakeLauncher::default())
}
fn fake_defaults() -> Arc<Defaults> {
    Arc::new(FakeDefaults::default())
}
fn fake_appearance() -> Arc<Appearance> {
    Arc::new(FakeAppearance::default())
}

/// Wire every use case. Real adapters replace the `fake_*` calls one by one.
pub fn build_state(launch_file: Option<PathBuf>) -> AppState {
    let probe = fake_probe();
    let inspector = fake_inspector();
    let host = fake_host();
    let system = fake_system_packages();
    let user = fake_user_apps();
    let desktop = fake_desktop();
    let ledger = fake_ledger();
    let launcher = fake_launcher();
    let defaults = fake_defaults();
    let appearance = fake_appearance();
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
        cancel_install: CancelInstall::new(registry),
        list_installed: ListInstalledApps::new(user.clone(), ledger.clone(), system.clone()),
        uninstall_app: UninstallApp::new(system, user, desktop, ledger.clone()),
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
