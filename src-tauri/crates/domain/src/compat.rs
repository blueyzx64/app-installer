//! Can this package be installed on this machine?

use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

use crate::host::{Arch, DistroFamily, HostSystem};
use crate::package::{PackageFormat, PackageMetadata};
use crate::version::Version;

/// How the *package file's* version compares to the installed one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VersionRelation {
    Same,
    /// The file is older than what is installed (a downgrade).
    Older,
    /// The file is newer than what is installed (an update).
    Newer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Runtime {
    Fuse2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UnsupportedReason {
    AppImageType1,
    Dwarfs,
    UnknownFormat,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Compatibility {
    Ok,
    WrongFamily {
        made_for: DistroFamily,
    },
    WrongArch {
        package: Arch,
        host: Arch,
    },
    AlreadyInstalled {
        relation: VersionRelation,
        installed: Version,
    },
    MissingRuntime {
        what: Runtime,
    },
    Unsupported {
        reason: UnsupportedReason,
    },
}

impl Compatibility {
    /// Decide compatibility. `installed` is the version already on the machine, if any.
    ///
    /// Order matters: things the user cannot fix come first (family, arch, runtime),
    /// then the "already installed" notice.
    pub fn evaluate(
        meta: &PackageMetadata,
        host: &HostSystem,
        installed: Option<&Version>,
    ) -> Compatibility {
        if let Some(made_for) = meta.format.family()
            && made_for != host.family
        {
            return Compatibility::WrongFamily { made_for };
        }
        if !meta.arch.runs_on(&host.arch) {
            return Compatibility::WrongArch {
                package: meta.arch.clone(),
                host: host.arch.clone(),
            };
        }
        if meta.format == PackageFormat::AppImage && meta.needs_fuse2 && !host.has_fuse2 {
            return Compatibility::MissingRuntime {
                what: Runtime::Fuse2,
            };
        }
        if let Some(installed) = installed {
            let relation = match meta.version.cmp(installed) {
                Ordering::Equal => VersionRelation::Same,
                Ordering::Less => VersionRelation::Older,
                Ordering::Greater => VersionRelation::Newer,
            };
            return Compatibility::AlreadyInstalled {
                relation,
                installed: installed.clone(),
            };
        }
        Compatibility::Ok
    }

    /// The install button may be used: fresh install, or an update.
    pub fn is_installable(&self) -> bool {
        matches!(
            self,
            Compatibility::Ok
                | Compatibility::AlreadyInstalled {
                    relation: VersionRelation::Newer,
                    ..
                }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::InstallScope;

    fn meta(format: PackageFormat, arch: Arch, version: &str, fuse2: bool) -> PackageMetadata {
        PackageMetadata {
            format,
            name: "lumen-notes".into(),
            display_name: "Lumen Notes".into(),
            version: Version::new(version),
            arch,
            summary: None,
            icon: None,
            scope: InstallScope::for_format(format),
            needs_fuse2: fuse2,
        }
    }

    fn host(family: DistroFamily, arch: Arch, fuse2: bool) -> HostSystem {
        HostSystem {
            family,
            distro_name: "Fedora Linux".into(),
            version_id: "45".into(),
            arch,
            has_fuse2: fuse2,
        }
    }

    #[test]
    fn evaluate_table() {
        use Compatibility as C;
        use DistroFamily::*;
        let ver = |s: &str| Some(Version::new(s));
        #[allow(clippy::type_complexity)]
        let rows: Vec<(
            &str,
            PackageMetadata,
            HostSystem,
            Option<Version>,
            Compatibility,
        )> = vec![
            (
                "rpm on fedora",
                meta(PackageFormat::Rpm, Arch::X86_64, "1.0", false),
                host(RpmBased, Arch::X86_64, true),
                None,
                C::Ok,
            ),
            (
                "deb on fedora",
                meta(PackageFormat::Deb, Arch::X86_64, "1.0", false),
                host(RpmBased, Arch::X86_64, true),
                None,
                C::WrongFamily { made_for: DebBased },
            ),
            (
                "rpm on unknown distro",
                meta(PackageFormat::Rpm, Arch::X86_64, "1.0", false),
                host(Other, Arch::X86_64, true),
                None,
                C::WrongFamily { made_for: RpmBased },
            ),
            (
                "aarch64 on x86_64",
                meta(PackageFormat::Rpm, Arch::Aarch64, "1.0", false),
                host(RpmBased, Arch::X86_64, true),
                None,
                C::WrongArch {
                    package: Arch::Aarch64,
                    host: Arch::X86_64,
                },
            ),
            (
                "noarch anywhere",
                meta(PackageFormat::Rpm, Arch::Noarch, "1.0", false),
                host(RpmBased, Arch::Aarch64, true),
                None,
                C::Ok,
            ),
            (
                "family wins over arch",
                meta(PackageFormat::Deb, Arch::Aarch64, "1.0", false),
                host(RpmBased, Arch::X86_64, true),
                None,
                C::WrongFamily { made_for: DebBased },
            ),
            (
                "appimage anywhere",
                meta(PackageFormat::AppImage, Arch::X86_64, "1.0", false),
                host(DebBased, Arch::X86_64, false),
                None,
                C::Ok,
            ),
            (
                "appimage needs fuse2, missing",
                meta(PackageFormat::AppImage, Arch::X86_64, "1.0", true),
                host(RpmBased, Arch::X86_64, false),
                None,
                C::MissingRuntime {
                    what: Runtime::Fuse2,
                },
            ),
            (
                "appimage needs fuse2, present",
                meta(PackageFormat::AppImage, Arch::X86_64, "1.0", true),
                host(RpmBased, Arch::X86_64, true),
                None,
                C::Ok,
            ),
            (
                "static runtime (needs_fuse2 false) on host without fuse2",
                meta(PackageFormat::AppImage, Arch::X86_64, "1.0", false),
                host(RpmBased, Arch::X86_64, false),
                None,
                C::Ok,
            ),
            (
                "missing fuse2 beats already installed",
                meta(PackageFormat::AppImage, Arch::X86_64, "2.0", true),
                host(RpmBased, Arch::X86_64, false),
                ver("1.0"),
                C::MissingRuntime {
                    what: Runtime::Fuse2,
                },
            ),
            (
                "wrong arch beats missing fuse2",
                meta(PackageFormat::AppImage, Arch::Aarch64, "1.0", true),
                host(RpmBased, Arch::X86_64, false),
                None,
                C::WrongArch {
                    package: Arch::Aarch64,
                    host: Arch::X86_64,
                },
            ),
            (
                "same version",
                meta(PackageFormat::Rpm, Arch::X86_64, "1.0-1", false),
                host(RpmBased, Arch::X86_64, true),
                ver("1.0-1"),
                C::AlreadyInstalled {
                    relation: VersionRelation::Same,
                    installed: Version::new("1.0-1"),
                },
            ),
            (
                "update",
                meta(PackageFormat::Rpm, Arch::X86_64, "1.10", false),
                host(RpmBased, Arch::X86_64, true),
                ver("1.9"),
                C::AlreadyInstalled {
                    relation: VersionRelation::Newer,
                    installed: Version::new("1.9"),
                },
            ),
            (
                "downgrade",
                meta(PackageFormat::Rpm, Arch::X86_64, "1.0~rc1", false),
                host(RpmBased, Arch::X86_64, true),
                ver("1.0"),
                C::AlreadyInstalled {
                    relation: VersionRelation::Older,
                    installed: Version::new("1.0"),
                },
            ),
            (
                "wrong arch beats installed",
                meta(PackageFormat::Rpm, Arch::Aarch64, "1.0", false),
                host(RpmBased, Arch::X86_64, true),
                ver("1.0"),
                C::WrongArch {
                    package: Arch::Aarch64,
                    host: Arch::X86_64,
                },
            ),
        ];
        for (label, m, h, installed, want) in rows {
            assert_eq!(
                Compatibility::evaluate(&m, &h, installed.as_ref()),
                want,
                "{label}"
            );
        }
    }

    #[test]
    fn installable_only_when_ok_or_update() {
        use Compatibility as C;
        let v = Version::new("1");
        let rel = |relation| C::AlreadyInstalled {
            relation,
            installed: v.clone(),
        };
        assert!(C::Ok.is_installable());
        assert!(rel(VersionRelation::Newer).is_installable());
        assert!(!rel(VersionRelation::Same).is_installable());
        assert!(!rel(VersionRelation::Older).is_installable());
        assert!(
            !C::MissingRuntime {
                what: Runtime::Fuse2
            }
            .is_installable()
        );
        assert!(
            !C::Unsupported {
                reason: UnsupportedReason::Dwarfs
            }
            .is_installable()
        );
    }

    #[test]
    fn serde_shape_matches_ipc_tags() {
        let c = Compatibility::MissingRuntime {
            what: Runtime::Fuse2,
        };
        assert_eq!(
            serde_json::to_string(&c).unwrap(),
            r#"{"kind":"missingRuntime","what":"fuse2"}"#
        );
    }
}
