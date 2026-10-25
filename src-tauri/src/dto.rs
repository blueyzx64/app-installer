//! Wire types shared with the frontend (contracts v1, `docs/architecture/ipc-contract.md`).
//!
//! Rust names equal the generated TypeScript names. Every type derives
//! `specta::Type`, so `bindings.ts` is generated from here (ADR-0009).
//! Domain types never cross the boundary; each DTO has a `From<domain::X>`.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use installer_domain as domain;
use serde::{Deserialize, Serialize};
use specta::Type;
use specta_typescript::Number;

use crate::error::ErrorCode;

// ---------- package preview ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum PackageFormat {
    Rpm,
    Deb,
    AppImage,
}

impl From<domain::PackageFormat> for PackageFormat {
    fn from(f: domain::PackageFormat) -> Self {
        match f {
            domain::PackageFormat::Rpm => Self::Rpm,
            domain::PackageFormat::Deb => Self::Deb,
            domain::PackageFormat::AppImage => Self::AppImage,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum InstallScope {
    System,
    User,
}

impl From<domain::InstallScope> for InstallScope {
    fn from(s: domain::InstallScope) -> Self {
        match s {
            domain::InstallScope::System => Self::System,
            domain::InstallScope::User => Self::User,
        }
    }
}

/// Which distro family a package was built for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum MadeFor {
    RpmBased,
    DebBased,
}

impl From<domain::DistroFamily> for MadeFor {
    fn from(f: domain::DistroFamily) -> Self {
        match f {
            domain::DistroFamily::DebBased => Self::DebBased,
            // `WrongFamily` is only built from `PackageFormat::family()`, which yields
            // RpmBased or DebBased. `Other` cannot occur there.
            domain::DistroFamily::RpmBased | domain::DistroFamily::Other => Self::RpmBased,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum VersionRelation {
    Same,
    Older,
    Newer,
}

impl From<domain::VersionRelation> for VersionRelation {
    fn from(r: domain::VersionRelation) -> Self {
        match r {
            domain::VersionRelation::Same => Self::Same,
            domain::VersionRelation::Older => Self::Older,
            domain::VersionRelation::Newer => Self::Newer,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Runtime {
    Fuse2,
}

impl From<domain::Runtime> for Runtime {
    fn from(r: domain::Runtime) -> Self {
        match r {
            domain::Runtime::Fuse2 => Self::Fuse2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum UnsupportedReason {
    AppImageType1,
    Dwarfs,
    UnknownFormat,
}

impl From<domain::UnsupportedReason> for UnsupportedReason {
    fn from(r: domain::UnsupportedReason) -> Self {
        match r {
            domain::UnsupportedReason::AppImageType1 => Self::AppImageType1,
            domain::UnsupportedReason::Dwarfs => Self::Dwarfs,
            domain::UnsupportedReason::UnknownFormat => Self::UnknownFormat,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Compatibility {
    Ok,
    #[serde(rename_all = "camelCase")]
    WrongFamily {
        made_for: MadeFor,
    },
    #[serde(rename_all = "camelCase")]
    WrongArch {
        package_arch: String,
        host_arch: String,
    },
    #[serde(rename_all = "camelCase")]
    AlreadyInstalled {
        relation: VersionRelation,
        installed_version: String,
    },
    #[serde(rename_all = "camelCase")]
    MissingRuntime {
        what: Runtime,
    },
    #[serde(rename_all = "camelCase")]
    Unsupported {
        reason: UnsupportedReason,
    },
}

impl From<domain::Compatibility> for Compatibility {
    fn from(c: domain::Compatibility) -> Self {
        use domain::Compatibility as C;
        match c {
            C::Ok => Self::Ok,
            C::WrongFamily { made_for } => Self::WrongFamily {
                made_for: made_for.into(),
            },
            C::WrongArch { package, host } => Self::WrongArch {
                package_arch: package.as_str().to_owned(),
                host_arch: host.as_str().to_owned(),
            },
            C::AlreadyInstalled {
                relation,
                installed,
            } => Self::AlreadyInstalled {
                relation: relation.into(),
                installed_version: installed.as_str().to_owned(),
            },
            C::MissingRuntime { what } => Self::MissingRuntime { what: what.into() },
            C::Unsupported { reason } => Self::Unsupported {
                reason: reason.into(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PackagePreview {
    pub id: String,
    /// Machine name, e.g. `lumen-notes`.
    pub name: String,
    pub format: PackageFormat,
    pub display_name: String,
    pub version: String,
    pub summary: Option<String>,
    pub icon_data_url: Option<String>,
    // Packages are capped at 8 GiB, far below 2^53, so `number` is lossless.
    #[specta(type = Number)]
    pub size_bytes: u64,
    pub scope: InstallScope,
    pub needs_permission: bool,
    pub compat: Compatibility,
}

impl From<domain::PackagePreview> for PackagePreview {
    fn from(p: domain::PackagePreview) -> Self {
        Self {
            id: p.id.to_string(),
            name: p.meta.name,
            format: p.meta.format.into(),
            display_name: p.meta.display_name,
            version: p.meta.version.as_str().to_owned(),
            summary: p.meta.summary,
            icon_data_url: p.meta.icon.as_ref().map(icon_data_url),
            size_bytes: p.size_bytes,
            scope: p.meta.scope.into(),
            needs_permission: p.needs_permission,
            compat: p.compat.into(),
        }
    }
}

/// `data:` URL for an icon read from a package.
pub fn icon_data_url(icon: &domain::IconData) -> String {
    let mime = match icon.kind {
        domain::IconKind::Png => "image/png",
        domain::IconKind::Svg => "image/svg+xml",
    };
    format!("data:{mime};base64,{}", STANDARD.encode(&icon.bytes))
}

// ---------- install progress and outcomes ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum InstallPhase {
    Preparing,
    WaitingForPermission,
    Downloading,
    Installing,
    Finishing,
}

impl From<domain::InstallPhase> for InstallPhase {
    fn from(p: domain::InstallPhase) -> Self {
        match p {
            domain::InstallPhase::Preparing => Self::Preparing,
            domain::InstallPhase::WaitingForPermission => Self::WaitingForPermission,
            domain::InstallPhase::Downloading => Self::Downloading,
            domain::InstallPhase::Installing => Self::Installing,
            domain::InstallPhase::Finishing => Self::Finishing,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum InstallEvent {
    #[serde(rename_all = "camelCase")]
    Plan {
        extra_packages: u32,
        // Download sizes stay far below 2^53.
        #[specta(type = Number)]
        download_bytes: u64,
    },
    #[serde(rename_all = "camelCase")]
    Progress {
        phase: InstallPhase,
        percent: Option<u8>,
        eta_secs: Option<u32>,
        /// True only while the backend accepts a cancel (F-28).
        allow_cancel: bool,
    },
}

impl From<&domain::InstallPlan> for InstallEvent {
    fn from(p: &domain::InstallPlan) -> Self {
        Self::Plan {
            extra_packages: p.extra_packages,
            download_bytes: p.download_bytes,
        }
    }
}

impl From<domain::InstallProgress> for InstallEvent {
    fn from(p: domain::InstallProgress) -> Self {
        Self::Progress {
            phase: p.phase.into(),
            percent: p.percent,
            eta_secs: p.eta_secs,
            allow_cancel: p.allow_cancel,
        }
    }
}

/// What removing an app would do (F-42).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UninstallPlan {
    pub also_removes: u32,
    pub needs_permission: bool,
}

impl From<domain::UninstallPlan> for UninstallPlan {
    fn from(p: domain::UninstallPlan) -> Self {
        Self {
            also_removes: p.also_removes,
            needs_permission: p.needs_permission,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum AppSource {
    AppImage,
    SystemPackage,
}

impl From<domain::AppSource> for AppSource {
    fn from(s: domain::AppSource) -> Self {
        match s {
            domain::AppSource::AppImage => Self::AppImage,
            domain::AppSource::SystemPackage => Self::SystemPackage,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct InstalledApp {
    pub id: String,
    pub display_name: String,
    pub version: String,
    pub icon_data_url: Option<String>,
    pub source: AppSource,
    /// ISO-8601, UTC.
    pub installed_at: String,
    pub can_launch: bool,
}

impl From<domain::InstalledApp> for InstalledApp {
    fn from(a: domain::InstalledApp) -> Self {
        Self {
            id: a.id.to_string(),
            display_name: a.display_name,
            version: a.version.as_str().to_owned(),
            icon_data_url: a.icon.as_ref().map(icon_data_url),
            source: a.source.into(),
            installed_at: iso8601_utc(a.installed_at),
            can_launch: a.desktop_id.is_some(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum InstallOutcome {
    Installed {
        app: InstalledApp,
    },
    Cancelled,
    #[serde(rename_all = "camelCase")]
    Failed {
        code: ErrorCode,
        detail: Option<String>,
    },
}

impl From<domain::InstallOutcome> for InstallOutcome {
    fn from(o: domain::InstallOutcome) -> Self {
        match o {
            domain::InstallOutcome::Installed(app) => Self::Installed { app: app.into() },
            domain::InstallOutcome::Cancelled => Self::Cancelled,
            domain::InstallOutcome::Failed(e) => {
                let (code, detail) = failure(&e);
                Self::Failed { code, detail }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UninstallOutcome {
    Removed,
    Cancelled,
    #[serde(rename_all = "camelCase")]
    Failed {
        code: ErrorCode,
        detail: Option<String>,
    },
}

impl From<domain::UninstallOutcome> for UninstallOutcome {
    fn from(o: domain::UninstallOutcome) -> Self {
        match o {
            domain::UninstallOutcome::Removed => Self::Removed,
            domain::UninstallOutcome::Cancelled => Self::Cancelled,
            domain::UninstallOutcome::Failed(e) => {
                let (code, detail) = failure(&e);
                Self::Failed { code, detail }
            }
        }
    }
}

/// Code and technical detail for a failed outcome. `Cancelled` never gets here
/// (outcomes map it to `Cancelled`), but is handled so the function is total.
fn failure(e: &domain::InstallError) -> (ErrorCode, Option<String>) {
    let code = e.code().map_or(ErrorCode::Unknown, ErrorCode::from);
    (code, e.detail().map(str::to_owned))
}

// ---------- defaults ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DefaultStatus {
    pub is_default: bool,
    pub asked_before: bool,
}

impl From<domain::DefaultStatus> for DefaultStatus {
    fn from(s: domain::DefaultStatus) -> Self {
        Self {
            is_default: s.is_default,
            asked_before: s.asked_before,
        }
    }
}

// ---------- appearance ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ColorScheme {
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ControlSide {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ControlButton {
    Close,
    Minimize,
    Maximize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WindowControls {
    pub side: ControlSide,
    pub buttons: Vec<ControlButton>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Appearance {
    pub color_scheme: ColorScheme,
    /// `#RRGGBB` from the portal, or null for the default blue.
    pub accent: Option<String>,
    pub high_contrast: bool,
    pub reduced_motion: bool,
    pub controls: WindowControls,
}

impl From<domain::Appearance> for Appearance {
    fn from(a: domain::Appearance) -> Self {
        use domain::{ColorScheme as Cs, ControlButton as Cb, ControlSide as Side};
        Self {
            color_scheme: match a.color_scheme {
                Cs::Light => ColorScheme::Light,
                Cs::Dark => ColorScheme::Dark,
            },
            accent: a.accent.map(|c| c.to_hex()),
            high_contrast: a.high_contrast,
            reduced_motion: a.reduced_motion,
            controls: WindowControls {
                side: match a.controls.side {
                    Side::Left => ControlSide::Left,
                    Side::Right => ControlSide::Right,
                },
                buttons: a
                    .controls
                    .buttons
                    .into_iter()
                    .map(|b| match b {
                        Cb::Close => ControlButton::Close,
                        Cb::Minimize => ControlButton::Minimize,
                        Cb::Maximize => ControlButton::Maximize,
                    })
                    .collect(),
            },
        }
    }
}

// ---------- helpers ----------

/// Unix seconds to `YYYY-MM-DDTHH:MM:SSZ` (proleptic Gregorian, UTC).
pub fn iso8601_utc(unix_secs: i64) -> String {
    let days = unix_secs.div_euclid(86_400);
    let rem = unix_secs.rem_euclid(86_400);
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3_600,
        rem % 3_600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;
    use serde_json::json;

    fn j(v: &impl Serialize) -> serde_json::Value {
        serde_json::to_value(v).unwrap()
    }

    #[test]
    fn iso8601_known_values() {
        assert_eq!(iso8601_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso8601_utc(1_700_000_000), "2023-11-14T22:13:20Z");
        assert_eq!(iso8601_utc(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(iso8601_utc(-1), "1969-12-31T23:59:59Z");
    }

    #[test]
    fn compatibility_snapshots() {
        assert_eq!(j(&Compatibility::Ok), json!({"kind": "ok"}));
        assert_eq!(
            j(&Compatibility::WrongFamily {
                made_for: MadeFor::DebBased
            }),
            json!({"kind": "wrongFamily", "madeFor": "debBased"})
        );
        assert_eq!(
            j(&Compatibility::WrongArch {
                package_arch: "aarch64".into(),
                host_arch: "x86_64".into()
            }),
            json!({"kind": "wrongArch", "packageArch": "aarch64", "hostArch": "x86_64"})
        );
        assert_eq!(
            j(&Compatibility::AlreadyInstalled {
                relation: VersionRelation::Older,
                installed_version: "2.0".into()
            }),
            json!({"kind": "alreadyInstalled", "relation": "older", "installedVersion": "2.0"})
        );
        assert_eq!(
            j(&Compatibility::MissingRuntime {
                what: Runtime::Fuse2
            }),
            json!({"kind": "missingRuntime", "what": "fuse2"})
        );
        assert_eq!(
            j(&Compatibility::Unsupported {
                reason: UnsupportedReason::AppImageType1
            }),
            json!({"kind": "unsupported", "reason": "appImageType1"})
        );
    }

    #[test]
    fn compatibility_maps_from_domain() {
        let c = domain::Compatibility::AlreadyInstalled {
            relation: domain::VersionRelation::Newer,
            installed: domain::Version::new("1.0"),
        };
        assert_eq!(
            j(&Compatibility::from(c)),
            json!({"kind": "alreadyInstalled", "relation": "newer", "installedVersion": "1.0"})
        );
        let c = domain::Compatibility::WrongArch {
            package: domain::Arch::Aarch64,
            host: domain::Arch::X86_64,
        };
        assert_eq!(
            j(&Compatibility::from(c)),
            json!({"kind": "wrongArch", "packageArch": "aarch64", "hostArch": "x86_64"})
        );
    }

    #[test]
    fn install_event_snapshots() {
        assert_eq!(
            j(&InstallEvent::Plan {
                extra_packages: 2,
                download_bytes: 1024
            }),
            json!({"kind": "plan", "extraPackages": 2, "downloadBytes": 1024})
        );
        assert_eq!(
            j(&InstallEvent::Progress {
                phase: InstallPhase::WaitingForPermission,
                percent: None,
                eta_secs: Some(3),
                allow_cancel: true
            }),
            json!({"kind": "progress", "phase": "waitingForPermission", "percent": null, "etaSecs": 3, "allowCancel": true})
        );
        assert_eq!(
            j(&UninstallPlan {
                also_removes: 2,
                needs_permission: true
            }),
            json!({"alsoRemoves": 2, "needsPermission": true})
        );
    }

    fn app() -> InstalledApp {
        InstalledApp {
            id: "lumen-notes".into(),
            display_name: "Lumen Notes".into(),
            version: "1.0".into(),
            icon_data_url: None,
            source: AppSource::SystemPackage,
            installed_at: "1970-01-01T00:00:00Z".into(),
            can_launch: false,
        }
    }

    #[test]
    fn install_outcome_snapshots() {
        assert_eq!(
            j(&InstallOutcome::Installed { app: app() }),
            json!({"kind": "installed", "app": {
                "id": "lumen-notes", "displayName": "Lumen Notes", "version": "1.0",
                "iconDataUrl": null, "source": "systemPackage",
                "installedAt": "1970-01-01T00:00:00Z", "canLaunch": false}})
        );
        assert_eq!(j(&InstallOutcome::Cancelled), json!({"kind": "cancelled"}));
        assert_eq!(
            j(&InstallOutcome::Failed {
                code: ErrorCode::Offline,
                detail: None
            }),
            json!({"kind": "failed", "code": "offline", "detail": null})
        );
    }

    #[test]
    fn outcomes_map_from_domain() {
        use domain::InstallError as E;
        assert_eq!(
            j(&InstallOutcome::from(domain::InstallOutcome::Cancelled)),
            json!({"kind": "cancelled"})
        );
        assert_eq!(
            j(&InstallOutcome::from(domain::InstallOutcome::Failed(
                E::Unknown("boom".into())
            ))),
            json!({"kind": "failed", "code": "unknown", "detail": "boom"})
        );
        assert_eq!(
            j(&UninstallOutcome::from(domain::UninstallOutcome::Removed)),
            json!({"kind": "removed"})
        );
        assert_eq!(
            j(&UninstallOutcome::from(domain::UninstallOutcome::Cancelled)),
            json!({"kind": "cancelled"})
        );
        assert_eq!(
            j(&UninstallOutcome::from(domain::UninstallOutcome::Failed(
                E::PermissionDenied
            ))),
            json!({"kind": "failed", "code": "permissionDenied", "detail": null})
        );
    }

    #[test]
    fn appearance_maps_hex_accent() {
        let a = domain::Appearance {
            accent: Some(domain::Rgb {
                r: 0,
                g: 128,
                b: 255,
            }),
            ..domain::Appearance::default()
        };
        assert_eq!(
            j(&Appearance::from(a)),
            json!({"colorScheme": "light", "accent": "#0080FF", "highContrast": false,
                   "reducedMotion": false,
                   "controls": {"side": "right", "buttons": ["close"]}})
        );
    }

    #[test]
    fn icon_becomes_data_url() {
        let icon = domain::IconData {
            kind: domain::IconKind::Png,
            bytes: vec![1, 2, 3],
        };
        assert_eq!(icon_data_url(&icon), "data:image/png;base64,AQID");
    }
}
