//! One struct per use case, generic over the ports it needs.

mod appearance;
mod cancel_install;
mod default_handler;
mod install_package;
mod launch_app;
mod list_installed;
mod open_package;
mod uninstall_app;
mod uninstall_plan; // lane S1b

pub use appearance::{GetAppearance, WatchAppearance};
pub use cancel_install::CancelInstall;
pub use default_handler::{DismissDefaultPrompt, GetDefaultHandlerStatus, MakeDefaultHandler};
pub use install_package::InstallPackage;
pub use launch_app::LaunchApp;
pub use list_installed::ListInstalledApps;
pub use open_package::OpenPackage;
pub use uninstall_app::UninstallApp;
pub use uninstall_plan::UninstallPlan; // lane S1b
