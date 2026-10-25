//! The machine we run on, and the CPU architecture vocabulary.

use std::fmt;

use serde::{Deserialize, Serialize};

/// CPU architecture. `Noarch` means "runs anywhere" (rpm `noarch`, deb `all`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Arch {
    X86_64,
    Aarch64,
    Armv7,
    I686,
    Noarch,
    Other(String),
}

impl Arch {
    /// Parse rpm (`x86_64`), deb (`amd64`), ELF/uname (`aarch64`, `arm64`) spellings.
    pub fn parse(raw: &str) -> Arch {
        match raw.trim().to_ascii_lowercase().as_str() {
            "x86_64" | "amd64" | "x64" => Arch::X86_64,
            "aarch64" | "arm64" => Arch::Aarch64,
            "armv7hl" | "armv7l" | "armhf" | "arm" => Arch::Armv7,
            "i386" | "i486" | "i586" | "i686" | "x86" => Arch::I686,
            "noarch" | "all" | "any" => Arch::Noarch,
            other => Arch::Other(other.to_owned()),
        }
    }

    /// The rpm-style name, as shown in `wrongArch` details.
    pub fn as_str(&self) -> &str {
        match self {
            Arch::X86_64 => "x86_64",
            Arch::Aarch64 => "aarch64",
            Arch::Armv7 => "armv7hl",
            Arch::I686 => "i686",
            Arch::Noarch => "noarch",
            Arch::Other(s) => s,
        }
    }

    /// Whether a package built for `self` runs on `host`.
    pub fn runs_on(&self, host: &Arch) -> bool {
        matches!(self, Arch::Noarch) || self == host
    }
}

impl fmt::Display for Arch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DistroFamily {
    RpmBased,
    DebBased,
    Other,
}

impl DistroFamily {
    /// Derive the family from os-release `ID` and `ID_LIKE` (space separated).
    pub fn from_os_release(id: &str, id_like: &str) -> DistroFamily {
        let words = || id.split_whitespace().chain(id_like.split_whitespace());
        let has =
            |names: &[&str]| words().any(|w| names.contains(&w.to_ascii_lowercase().as_str()));
        if has(&[
            "fedora",
            "rhel",
            "centos",
            "rocky",
            "almalinux",
            "suse",
            "opensuse",
            "opensuse-leap",
            "opensuse-tumbleweed",
            "mageia",
        ]) {
            DistroFamily::RpmBased
        } else if has(&["debian", "ubuntu", "linuxmint", "pop", "raspbian"]) {
            DistroFamily::DebBased
        } else {
            DistroFamily::Other
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostSystem {
    pub family: DistroFamily,
    /// Human name, e.g. "Fedora Linux".
    pub distro_name: String,
    /// os-release `VERSION_ID`, e.g. "45".
    pub version_id: String,
    pub arch: Arch,
    /// `libfuse.so.2` present (needed by legacy AppImage runtimes).
    pub has_fuse2: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arch_parse_table() {
        let rows = [
            ("x86_64", Arch::X86_64),
            ("amd64", Arch::X86_64),
            ("AMD64", Arch::X86_64),
            ("aarch64", Arch::Aarch64),
            ("arm64", Arch::Aarch64),
            ("armhf", Arch::Armv7),
            ("i386", Arch::I686),
            ("noarch", Arch::Noarch),
            ("all", Arch::Noarch),
            ("riscv64", Arch::Other("riscv64".into())),
        ];
        for (raw, want) in rows {
            assert_eq!(Arch::parse(raw), want, "{raw}");
        }
    }

    #[test]
    fn noarch_runs_everywhere_others_only_on_match() {
        assert!(Arch::Noarch.runs_on(&Arch::Aarch64));
        assert!(Arch::X86_64.runs_on(&Arch::X86_64));
        assert!(!Arch::Aarch64.runs_on(&Arch::X86_64));
    }

    #[test]
    fn family_from_os_release_table() {
        let rows = [
            ("fedora", "", DistroFamily::RpmBased),
            ("rocky", "rhel centos fedora", DistroFamily::RpmBased),
            ("ubuntu", "debian", DistroFamily::DebBased),
            ("pop", "ubuntu debian", DistroFamily::DebBased),
            ("arch", "", DistroFamily::Other),
            ("nixos", "", DistroFamily::Other),
        ];
        for (id, like, want) in rows {
            assert_eq!(
                DistroFamily::from_os_release(id, like),
                want,
                "{id} / {like}"
            );
        }
    }
}
