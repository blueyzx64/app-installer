//! Pure domain model and rules. No IO, no async, no framework types.
//!
//! See `docs/architecture/ARCHITECTURE.md` section 5.
#![forbid(unsafe_code)]

pub mod app;
pub mod appearance;
pub mod compat;
pub mod error;
pub mod host;
pub mod install;
pub mod package;
pub mod preview;
pub mod version;

pub use app::{AppId, AppSource, InstalledApp, InvalidAppId, nfc};
pub use appearance::{
    Appearance, ColorScheme, ControlButton, ControlSide, Rgb, WindowControls, button_layout,
};
pub use compat::{Compatibility, Runtime, UnsupportedReason, VersionRelation};
pub use error::{BackendError, ErrorCode, InspectError, InstallError, UserAction};
pub use host::{Arch, DistroFamily, HostSystem};
pub use install::{
    DefaultStatus, InstallOutcome, InstallPhase, InstallPlan, InstallProgress, UninstallOutcome,
    UninstallPlan,
};
pub use package::{
    IconData, IconKind, InstallScope, PackageFile, PackageFormat, PackageMetadata, SNIFF_LEN,
};
pub use preview::{InvalidPreviewId, PackagePreview, PreviewId};
pub use version::Version;
