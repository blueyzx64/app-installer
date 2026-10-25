//! R3 review proofs for installer-domain. Vectors in review_r3_vectors.rs come from
//! the real `rpm` 4.18.2 (`rpm --eval '%{lua: rpm.vercmp(a,b)}'`), not from our port.
use std::cmp::Ordering::{self, Equal, Greater, Less};

use installer_domain::version::rpmvercmp;
use installer_domain::*;

include!("review_r3_vectors.rs");

fn ord(i: i8) -> Ordering {
    match i {
        0 => Equal,
        x if x < 0 => Less,
        _ => Greater,
    }
}

#[test]
fn review_r3_rpmvercmp_matches_real_rpm_vectors() {
    assert!(RPM_VECTORS.len() >= 40);
    let mut bad = vec![];
    for (a, b, want) in RPM_VECTORS {
        if rpmvercmp(a, b) != ord(*want) {
            bad.push(format!(
                "{a:?} vs {b:?}: rpm={want} ours={:?}",
                rpmvercmp(a, b)
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "{} mismatches:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

#[test]
fn review_r3_version_epoch_and_release_vectors() {
    let v = Version::new;
    let rows = [
        ("1:1.0", "2.0", Greater),
        ("0:1.0-1", "1.0-1", Equal),
        ("1.0-1.fc45", "1.0-2.fc45", Less),
        ("1.0-10.fc45", "1.0-9.fc45", Greater),
        ("2:1.0-1", "1:9.9-9", Greater),
        ("1.0~rc1-1", "1.0-1", Less),
        ("1.0^git1-1", "1.0-1", Greater),
        ("1.2.3-4.fc45", "1.2.3-4.fc44", Greater),
    ];
    for (a, b, want) in rows {
        assert_eq!(v(a).cmp(&v(b)), want, "{a} vs {b}");
    }
}

fn meta(format: PackageFormat, arch: Arch, version: &str) -> PackageMetadata {
    PackageMetadata {
        format,
        name: "lumen-notes".into(),
        display_name: "Lumen Notes".into(),
        version: Version::new(version),
        arch,
        summary: None,
        icon: None,
        scope: InstallScope::for_format(format),
        needs_fuse2: false,
    }
}
fn host(family: DistroFamily, arch: Arch) -> HostSystem {
    HostSystem {
        family,
        distro_name: "x".into(),
        version_id: String::new(),
        arch,
        has_fuse2: true,
    }
}

#[test]
fn review_r3_family_matrix_f10() {
    use DistroFamily::*;
    let deb_on_fedora = Compatibility::evaluate(
        &meta(PackageFormat::Deb, Arch::X86_64, "1"),
        &host(RpmBased, Arch::X86_64),
        None,
    );
    assert_eq!(
        deb_on_fedora,
        Compatibility::WrongFamily { made_for: DebBased }
    );
    let rpm_on_ubuntu = Compatibility::evaluate(
        &meta(PackageFormat::Rpm, Arch::X86_64, "1"),
        &host(DebBased, Arch::X86_64),
        None,
    );
    assert_eq!(
        rpm_on_ubuntu,
        Compatibility::WrongFamily { made_for: RpmBased }
    );
    let v = Version::new("1");
    assert!(matches!(
        Compatibility::evaluate(
            &meta(PackageFormat::Deb, Arch::X86_64, "1"),
            &host(RpmBased, Arch::X86_64),
            Some(&v)
        ),
        Compatibility::WrongFamily { .. }
    ));
}

#[test]
fn review_r3_arch_matrix_f11() {
    let ev = |pkg: &str, host_arch: &str| {
        Compatibility::evaluate(
            &meta(PackageFormat::Rpm, Arch::parse(pkg), "1"),
            &host(DistroFamily::RpmBased, Arch::parse(host_arch)),
            None,
        )
    };
    assert_eq!(ev("noarch", "x86_64"), Compatibility::Ok);
    assert_eq!(ev("noarch", "aarch64"), Compatibility::Ok);
    assert_eq!(ev("noarch", "armv7hl"), Compatibility::Ok);
    assert_eq!(ev("armv7hl", "armv7l"), Compatibility::Ok);
    assert_eq!(
        ev("armv7hl", "arm"),
        Compatibility::Ok,
        "Rust ARCH on 32-bit arm"
    );
    assert_eq!(ev("x86_64", "x86_64"), Compatibility::Ok);
    assert_eq!(ev("amd64", "x86_64"), Compatibility::Ok);
    assert!(matches!(
        ev("i686", "x86_64"),
        Compatibility::WrongArch { .. }
    ));
    assert!(matches!(
        ev("x86_64", "i686"),
        Compatibility::WrongArch { .. }
    ));
    assert!(matches!(
        ev("aarch64", "x86_64"),
        Compatibility::WrongArch { .. }
    ));
    assert!(matches!(
        ev("armv7hl", "aarch64"),
        Compatibility::WrongArch { .. }
    ));
}

#[test]
fn review_r3_installed_mapping_f12_f13_f14() {
    let ev = |pkg: &str, inst: &str| {
        Compatibility::evaluate(
            &meta(PackageFormat::Rpm, Arch::X86_64, pkg),
            &host(DistroFamily::RpmBased, Arch::X86_64),
            Some(&Version::new(inst)),
        )
    };
    let rel = |c: Compatibility| match c {
        Compatibility::AlreadyInstalled { relation, .. } => relation,
        other => panic!("{other:?}"),
    };
    assert_eq!(rel(ev("1.0-1.fc45", "1.0-1.fc45")), VersionRelation::Same);
    assert_eq!(rel(ev("1.0-2.fc45", "1.0-1.fc45")), VersionRelation::Newer);
    assert_eq!(rel(ev("1.0-1.fc45", "1.0-2.fc45")), VersionRelation::Older);
    assert_eq!(rel(ev("1:0.1-1", "9.9-1")), VersionRelation::Newer, "epoch");
    assert!(ev("1.0-2", "1.0-1").is_installable());
    assert!(!ev("1.0-1", "1.0-1").is_installable());
    assert!(!ev("1.0-1", "1.0-2").is_installable());
}

#[test]
fn review_r3_family_from_os_release_variants() {
    use DistroFamily::*;
    let rows = [
        ("rocky", "rhel centos fedora", RpmBased),
        ("amzn", "centos rhel fedora", RpmBased),
        ("opensuse-leap", "suse opensuse", RpmBased),
        ("sles", "suse opensuse", RpmBased),
        ("nobara", "rhel centos fedora", RpmBased),
        ("linuxmint", "ubuntu debian", DebBased),
        ("debian", "", DebBased),
        ("endeavouros", "arch", Other),
        ("", "", Other),
        ("FEDORA", "", RpmBased),
    ];
    for (id, like, want) in rows {
        assert_eq!(DistroFamily::from_os_release(id, like), want, "{id}/{like}");
    }
}

#[test]
fn review_r3_slug_collisions_for_non_ascii_names() {
    // Distinct non-latin app names must not share an AppId: ledger, install dir and
    // .desktop id all key on it.
    assert_ne!(
        AppId::from_name("日本語ノート"),
        AppId::from_name("中文笔记"),
        "distinct non-ASCII names both slug to {:?}",
        AppId::from_name("日本語ノート").as_str()
    );
}

#[test]
fn review_r3_slug_basics() {
    assert_eq!(AppId::from_name("Lumen Notes").as_str(), "lumen-notes");
    assert_eq!(AppId::from_name("../../etc/passwd").as_str(), "etc-passwd");
    assert_eq!(AppId::from_name(&"a".repeat(200)).as_str().len(), 64);
    let long = format!("{}-b", "a".repeat(63));
    assert!(!AppId::from_name(&long).as_str().ends_with('-'));
    for s in ["", "--", "\0", "/", "\u{2122}"] {
        assert_eq!(AppId::from_name(s).as_str(), "app");
    }
    assert!("Lumen".parse::<AppId>().is_err());
    assert!("a--b".parse::<AppId>().is_err());
}

#[test]
fn review_r3_sniff_html_and_truncated() {
    assert_eq!(
        PackageFormat::sniff(b"<!DOCTYPE html><html>", "x.rpm"),
        None
    );
    assert_eq!(PackageFormat::sniff(&[0xED, 0xAB, 0xEE], "x.rpm"), None);
    assert_eq!(
        PackageFormat::sniff(b"!<arch>\nfoo.o/          ", "x.deb"),
        None
    );
}
