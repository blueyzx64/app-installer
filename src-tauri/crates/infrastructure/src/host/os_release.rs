//! Real `HostProbe`: reads os-release, the CPU arch and looks for libfuse2.
//! Reads files only. Never starts a process.

use std::fs;
use std::path::PathBuf;

use installer_application::ports::HostProbe;
use installer_domain::{Arch, DistroFamily, HostSystem};

const DEFAULT_OS_RELEASE: [&str; 2] = ["/etc/os-release", "/usr/lib/os-release"];

const DEFAULT_LIB_DIRS: [&str; 10] = [
    "/usr/lib64",
    "/usr/lib",
    "/lib64",
    "/lib",
    "/usr/lib/x86_64-linux-gnu",
    "/lib/x86_64-linux-gnu",
    "/usr/lib/aarch64-linux-gnu",
    "/lib/aarch64-linux-gnu",
    "/usr/lib/arm-linux-gnueabihf",
    "/usr/local/lib",
];

const FUSE2_LIB: &str = "libfuse.so.2";

/// The fields we use from an os-release file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OsReleaseInfo {
    pub id: String,
    pub id_like: String,
    pub version_id: String,
    pub name: String,
}

impl OsReleaseInfo {
    /// Parse `KEY=value` lines. Quotes are removed; comments and unknown keys are skipped.
    pub fn parse(text: &str) -> Self {
        let mut info = Self::default();
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('#') {
                continue;
            }
            let Some((key, raw)) = line.split_once('=') else {
                continue;
            };
            let value = unquote(raw.trim());
            match key.trim() {
                "ID" => info.id = value,
                "ID_LIKE" => info.id_like = value,
                "VERSION_ID" => info.version_id = value,
                "NAME" => info.name = value,
                _ => {}
            }
        }
        info
    }
}

fn unquote(raw: &str) -> String {
    let quoted = raw.len() >= 2
        && ((raw.starts_with('"') && raw.ends_with('"'))
            || (raw.starts_with('\'') && raw.ends_with('\'')));
    if !quoted {
        return raw.to_owned();
    }
    let inner = &raw[1..raw.len() - 1];
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some(n @ ('"' | '\\' | '$' | '`')) => out.push(n),
                Some(n) => {
                    out.push('\\');
                    out.push(n);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Reads the host facts. Paths are injectable so tests use temp dirs.
#[derive(Debug, Clone)]
pub struct OsReleaseProbe {
    os_release_paths: Vec<PathBuf>,
    lib_dirs: Vec<PathBuf>,
    arch: Arch,
}

impl Default for OsReleaseProbe {
    fn default() -> Self {
        Self {
            os_release_paths: DEFAULT_OS_RELEASE.iter().map(PathBuf::from).collect(),
            lib_dirs: DEFAULT_LIB_DIRS.iter().map(PathBuf::from).collect(),
            arch: Arch::parse(std::env::consts::ARCH),
        }
    }
}

impl OsReleaseProbe {
    /// The real machine.
    pub fn new() -> Self {
        Self::default()
    }

    /// Use these os-release files (first readable wins) and library dirs.
    pub fn with_roots(os_release_paths: Vec<PathBuf>, lib_dirs: Vec<PathBuf>) -> Self {
        Self {
            os_release_paths,
            lib_dirs,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn with_arch(mut self, arch: Arch) -> Self {
        self.arch = arch;
        self
    }

    fn read_os_release(&self) -> OsReleaseInfo {
        self.os_release_paths
            .iter()
            .find_map(|p| fs::read_to_string(p).ok())
            .map(|t| OsReleaseInfo::parse(&t))
            .unwrap_or_default()
    }

    fn has_fuse2(&self) -> bool {
        self.lib_dirs.iter().any(|d| d.join(FUSE2_LIB).exists())
    }
}

impl HostProbe for OsReleaseProbe {
    fn host(&self) -> HostSystem {
        let info = self.read_os_release();
        let distro_name = if info.name.is_empty() {
            "Linux".to_owned()
        } else {
            info.name.clone()
        };
        HostSystem {
            family: DistroFamily::from_os_release(&info.id, &info.id_like),
            distro_name,
            version_id: info.version_id,
            arch: self.arch.clone(),
            has_fuse2: self.has_fuse2(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct Tmp(PathBuf);
    impl Tmp {
        fn new() -> Self {
            static N: AtomicU32 = AtomicU32::new(0);
            let p = std::env::temp_dir().join(format!(
                "installer-host-{}-{}",
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&p).unwrap();
            Tmp(p)
        }
        fn file(&self, name: &str, text: &str) -> PathBuf {
            let p = self.0.join(name);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(&p, text).unwrap();
            p
        }
    }
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    const FEDORA: &str = "NAME=\"Fedora Linux\"\nVERSION=\"45 (Workstation Edition)\"\nID=fedora\nVERSION_ID=45\n# comment\nPRETTY_NAME=\"Fedora Linux 45\"\n";
    const ROCKY: &str =
        "NAME='Rocky Linux'\nID=\"rocky\"\nID_LIKE=\"rhel centos fedora\"\nVERSION_ID=\"9.4\"\n";

    #[test]
    fn parses_quotes_and_skips_noise() {
        let i = OsReleaseInfo::parse(FEDORA);
        assert_eq!(i.name, "Fedora Linux");
        assert_eq!(i.id, "fedora");
        assert_eq!(i.version_id, "45");
        assert_eq!(i.id_like, "");
        let r = OsReleaseInfo::parse(ROCKY);
        assert_eq!(r.name, "Rocky Linux");
        assert_eq!(r.id_like, "rhel centos fedora");
        assert_eq!(r.version_id, "9.4");
        assert_eq!(unquote(r#""a \"b\" \\ c""#), r#"a "b" \ c"#);
    }

    #[test]
    fn fedora_with_fuse2() {
        let t = Tmp::new();
        let os = t.file("etc/os-release", FEDORA);
        t.file("usr/lib64/libfuse.so.2", "");
        let probe = OsReleaseProbe::with_roots(vec![os], vec![t.0.join("usr/lib64")])
            .with_arch(Arch::X86_64);
        let h = probe.host();
        assert_eq!(h.family, DistroFamily::RpmBased);
        assert_eq!(h.distro_name, "Fedora Linux");
        assert_eq!(h.version_id, "45");
        assert_eq!(h.arch, Arch::X86_64);
        assert!(h.has_fuse2);
    }

    #[test]
    fn no_fuse2_when_lib_missing() {
        let t = Tmp::new();
        let os = t.file("os-release", FEDORA);
        t.file("lib/libfuse.so.3", "");
        let probe = OsReleaseProbe::with_roots(vec![os], vec![t.0.join("lib"), t.0.join("nope")]);
        assert!(!probe.host().has_fuse2);
    }

    #[test]
    fn falls_back_to_second_os_release_path() {
        let t = Tmp::new();
        let usr = t.file("usr/lib/os-release", ROCKY);
        let probe = OsReleaseProbe::with_roots(vec![t.0.join("etc/os-release"), usr], vec![]);
        let h = probe.host();
        assert_eq!(h.family, DistroFamily::RpmBased);
        assert_eq!(h.distro_name, "Rocky Linux");
    }

    #[test]
    fn missing_os_release_gives_unknown_linux() {
        let t = Tmp::new();
        let h = OsReleaseProbe::with_roots(vec![t.0.join("none")], vec![]).host();
        assert_eq!(h.family, DistroFamily::Other);
        assert_eq!(h.distro_name, "Linux");
        assert_eq!(h.version_id, "");
    }

    #[test]
    fn default_arch_is_from_build_target() {
        assert_eq!(
            OsReleaseProbe::new().host().arch,
            Arch::parse(std::env::consts::ARCH)
        );
    }
}
