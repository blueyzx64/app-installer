//! Port traits. Infrastructure implements them; use cases call them.

mod appearance;
mod desktop;
mod files;
mod host;
mod ledger;
mod packages;

pub use appearance::{AppearanceObserver, AppearanceSource};
pub use desktop::{DefaultHandlerRegistry, DesktopIntegration, Launcher};
pub use files::{FileHead, FileProbe, PackageInspector};
pub use host::{Clock, HostProbe};
pub use ledger::AppLedger;
pub use packages::{InstallObserver, SystemPackages, UserApps};
