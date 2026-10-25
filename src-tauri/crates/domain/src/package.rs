//! Package files and what we learn from reading them.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::host::{Arch, DistroFamily};
use crate::version::Version;

/// How many leading bytes `PackageFormat::sniff` needs. Read at least this much.
pub const SNIFF_LEN: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PackageFormat {
    Rpm,
    Deb,
    AppImage,
}

impl PackageFormat {
    const RPM_MAGIC: &'static [u8] = &[0xED, 0xAB, 0xEE, 0xDB];
    const DEB_MAGIC: &'static [u8] = b"!<arch>\ndebian-binary";
    const ELF_MAGIC: &'static [u8] = b"\x7fELF";

    /// Detect the format from the first bytes of a file.
    ///
    /// Magic bytes decide. The file name is only a hint and never overrides
    /// them: a `.rpm` that starts with ELF bytes is not an RPM, and a file with
    /// unknown bytes is `None` whatever its extension says.
    ///
    /// AppImage: ELF header plus `AI` and type `0x01` or `0x02` at offset 8.
    /// Type 1 is still an AppImage here; the inspector reports it as unsupported.
    pub fn sniff(header: &[u8], _file_name: &str) -> Option<PackageFormat> {
        if header.starts_with(Self::RPM_MAGIC) {
            Some(PackageFormat::Rpm)
        } else if header.starts_with(Self::DEB_MAGIC) {
            Some(PackageFormat::Deb)
        } else if header.starts_with(Self::ELF_MAGIC)
            && header.get(8..10) == Some(b"AI")
            && matches!(header.get(10), Some(1 | 2))
        {
            Some(PackageFormat::AppImage)
        } else {
            None
        }
    }

    /// Extension hint only (lower-case compare). Useful for messages, never for decisions.
    pub fn from_extension(file_name: &str) -> Option<PackageFormat> {
        let lower = file_name.to_ascii_lowercase();
        if lower.ends_with(".rpm") {
            Some(PackageFormat::Rpm)
        } else if lower.ends_with(".deb") {
            Some(PackageFormat::Deb)
        } else if lower.ends_with(".appimage") {
            Some(PackageFormat::AppImage)
        } else {
            None
        }
    }

    /// The distro family a package of this format is made for. AppImage runs anywhere.
    pub fn family(self) -> Option<DistroFamily> {
        match self {
            PackageFormat::Rpm => Some(DistroFamily::RpmBased),
            PackageFormat::Deb => Some(DistroFamily::DebBased),
            PackageFormat::AppImage => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InstallScope {
    /// Needs permission (polkit); installed by the system package manager.
    System,
    /// Per-user, no password.
    User,
}

impl InstallScope {
    pub fn for_format(format: PackageFormat) -> InstallScope {
        match format {
            PackageFormat::Rpm | PackageFormat::Deb => InstallScope::System,
            PackageFormat::AppImage => InstallScope::User,
        }
    }

    pub fn needs_permission(self) -> bool {
        self == InstallScope::System
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageFile {
    pub path: PathBuf,
    pub format: PackageFormat,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IconKind {
    Png,
    Svg,
}

/// Icon bytes read from a package. At most 256 px by convention; the shell builds the data URL.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IconData {
    pub kind: IconKind,
    pub bytes: Vec<u8>,
}

impl std::fmt::Debug for IconData {
    // Never dump file contents into logs.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "IconData({:?}, {} bytes)", self.kind, self.bytes.len())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageMetadata {
    pub format: PackageFormat,
    /// Machine name, e.g. `lumen-notes`.
    pub name: String,
    pub display_name: String,
    pub version: Version,
    pub arch: Arch,
    pub summary: Option<String>,
    pub icon: Option<IconData>,
    pub scope: InstallScope,
    /// AppImage with a legacy dynamic runtime that needs `libfuse.so.2`.
    pub needs_fuse2: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn appimage_header(kind: u8) -> Vec<u8> {
        let mut h = vec![0u8; SNIFF_LEN];
        h[..4].copy_from_slice(b"\x7fELF");
        h[8] = b'A';
        h[9] = b'I';
        h[10] = kind;
        h
    }

    fn deb_header() -> Vec<u8> {
        b"!<arch>\ndebian-binary   1234567890  0     0     100644  4         `\n2.0\n".to_vec()
    }

    #[test]
    fn sniff_table() {
        let rpm = [0xED, 0xAB, 0xEE, 0xDB, 3, 0, 0, 0];
        let mut elf_plain = vec![0u8; SNIFF_LEN];
        elf_plain[..4].copy_from_slice(b"\x7fELF");
        let rows: Vec<(&str, Vec<u8>, &str, Option<PackageFormat>)> = vec![
            ("rpm", rpm.to_vec(), "a.rpm", Some(PackageFormat::Rpm)),
            (
                "rpm wrong name",
                rpm.to_vec(),
                "a.txt",
                Some(PackageFormat::Rpm),
            ),
            ("deb", deb_header(), "a.deb", Some(PackageFormat::Deb)),
            (
                "appimage t2",
                appimage_header(2),
                "A.AppImage",
                Some(PackageFormat::AppImage),
            ),
            (
                "appimage t1",
                appimage_header(1),
                "A.AppImage",
                Some(PackageFormat::AppImage),
            ),
            (
                "appimage no ext",
                appimage_header(2),
                "A",
                Some(PackageFormat::AppImage),
            ),
            (
                "appimage type 3 unknown",
                appimage_header(3),
                "A.AppImage",
                None,
            ),
            ("plain elf", elf_plain, "tool.AppImage", None),
            (
                "rpm name, text bytes",
                b"hello world".to_vec(),
                "a.rpm",
                None,
            ),
            ("empty", vec![], "a.rpm", None),
            ("short rpm magic", vec![0xED, 0xAB], "a.rpm", None),
            ("deb magic cut", b"!<arch>\ndebian".to_vec(), "a.deb", None),
            (
                "rpm bytes, deb name",
                rpm.to_vec(),
                "a.deb",
                Some(PackageFormat::Rpm),
            ),
        ];
        for (label, header, name, want) in rows {
            assert_eq!(PackageFormat::sniff(&header, name), want, "{label}");
        }
    }

    #[test]
    fn extension_hint_is_case_insensitive() {
        assert_eq!(
            PackageFormat::from_extension("x.RPM"),
            Some(PackageFormat::Rpm)
        );
        assert_eq!(
            PackageFormat::from_extension("x.AppImage"),
            Some(PackageFormat::AppImage)
        );
        assert_eq!(PackageFormat::from_extension("x.zip"), None);
    }

    #[test]
    fn scope_for_format_table() {
        assert_eq!(
            InstallScope::for_format(PackageFormat::Rpm),
            InstallScope::System
        );
        assert_eq!(
            InstallScope::for_format(PackageFormat::Deb),
            InstallScope::System
        );
        assert_eq!(
            InstallScope::for_format(PackageFormat::AppImage),
            InstallScope::User
        );
        assert!(InstallScope::System.needs_permission());
        assert!(!InstallScope::User.needs_permission());
    }

    #[test]
    fn family_per_format() {
        assert_eq!(PackageFormat::Rpm.family(), Some(DistroFamily::RpmBased));
        assert_eq!(PackageFormat::Deb.family(), Some(DistroFamily::DebBased));
        assert_eq!(PackageFormat::AppImage.family(), None);
    }

    #[test]
    fn icon_debug_hides_bytes() {
        let icon = IconData {
            kind: IconKind::Png,
            bytes: vec![1, 2, 3],
        };
        assert_eq!(format!("{icon:?}"), "IconData(Png, 3 bytes)");
    }
}

#[cfg(test)]
mod prop {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn sniff_never_panics_and_rpm_needs_magic(
            header in proptest::collection::vec(any::<u8>(), 0..128),
            name_a in any::<String>(),
            name_b in any::<String>(),
        ) {
            let fmt = PackageFormat::sniff(&header, &name_a);
            if fmt == Some(PackageFormat::Rpm) {
                prop_assert!(header.starts_with(&[0xED, 0xAB, 0xEE, 0xDB]));
            }
            prop_assert_eq!(fmt, PackageFormat::sniff(&header, &name_b));
        }
    }
}
