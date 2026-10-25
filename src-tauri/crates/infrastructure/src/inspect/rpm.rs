//! `.rpm` inspector (F-05, F-08). Reads bytes only; never runs the package (ADR-0005).
//!
//! Metadata comes from the lead and the two headers (a few KiB, no payload read).
//! The icon (F-08) is the one step that touches the payload: a bounded, streaming
//! scan of the cpio archive. Any failure there yields `icon: None`, never an error.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;

use installer_application::ports::PackageInspector;
use installer_domain::{
    Arch, IconData, IconKind, InspectError, InstallScope, PackageFile, PackageFormat,
    PackageMetadata, UnsupportedReason, Version,
};

/// Decompressed payload bytes we are willing to walk while looking for an icon.
const PAYLOAD_SCAN_CAP: u64 = 96 * 1024 * 1024;
/// Most bytes lead + signature + header may span (real ones are well under 8 MiB).
const HEADER_BYTES_MAX: u64 = 64 * 1024 * 1024;
/// Largest single icon we keep.
const ICON_MAX_BYTES: u64 = 2 * 1024 * 1024;
/// Longest file name we accept inside the cpio archive.
const CPIO_NAME_MAX: u64 = 4096;

/// Inspects `.rpm` files with the `rpm` crate's header parser.
#[derive(Debug, Default, Clone, Copy)]
pub struct RpmInspector;

impl RpmInspector {
    pub fn new() -> Self {
        Self
    }

    /// Blocking worker behind [`PackageInspector::inspect`]. Public for tests and benches.
    pub fn inspect_path(path: &Path, with_icon: bool) -> Result<PackageMetadata, InspectError> {
        let file = File::open(path).map_err(map_io)?;
        let mut reader = BufReader::new(file);
        // Cap header reads: the crate trusts the sizes in the file. Hitting the cap is a short read.
        let parsed = {
            let mut capped = (&mut reader).take(HEADER_BYTES_MAX);
            rpm::PackageMetadata::parse(&mut capped).map_err(map_rpm)?
        };

        if parsed.is_source_package() {
            return Err(InspectError::Unsupported(UnsupportedReason::UnknownFormat));
        }
        let name = parsed.get_name().map_err(map_rpm)?.to_owned();
        if name.is_empty() {
            return Err(InspectError::Corrupt);
        }
        let version = parsed.get_version().map_err(map_rpm)?;
        let release = parsed.get_release().map_err(map_rpm)?;
        // A missing epoch tag is the normal case; epoch 0 and "none" are the same to rpm.
        let epoch = parsed.get_epoch().unwrap_or(0);
        let evr = if epoch > 0 {
            format!("{epoch}:{version}-{release}")
        } else {
            format!("{version}-{release}")
        };
        let arch = Arch::parse(parsed.get_arch().map_err(map_rpm)?);
        let summary = parsed
            .get_summary()
            .ok()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned);

        // `reader` now sits exactly on the payload (BufReader accounts for its buffer).
        let icon = if with_icon {
            read_icon(&mut reader, &name).ok().flatten()
        } else {
            None
        };

        Ok(PackageMetadata {
            format: PackageFormat::Rpm,
            display_name: display_name_from(&name),
            name,
            version: Version::new(evr),
            arch,
            summary,
            icon,
            scope: InstallScope::for_format(PackageFormat::Rpm),
            needs_fuse2: false,
        })
    }
}

impl PackageInspector for RpmInspector {
    fn supports(&self, format: PackageFormat) -> bool {
        format == PackageFormat::Rpm
    }

    async fn inspect(&self, file: &PackageFile) -> Result<PackageMetadata, InspectError> {
        if file.format != PackageFormat::Rpm {
            return Err(InspectError::Unsupported(UnsupportedReason::UnknownFormat));
        }
        let path = file.path.clone();
        tokio::task::spawn_blocking(move || Self::inspect_path(&path, true))
            .await
            .map_err(|e| InspectError::Io(e.to_string()))?
    }
}

/// `lumen-notes` -> `Lumen Notes`. A heuristic: split on `-`, `_`, `.` and capitalise.
pub(crate) fn display_name_from(name: &str) -> String {
    let words: Vec<String> = name
        .split(['-', '_', '.', ' '])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect(),
                None => String::new(),
            }
        })
        .collect();
    if words.is_empty() {
        name.to_owned()
    } else {
        words.join(" ")
    }
}

pub(crate) fn map_io(e: io::Error) -> InspectError {
    match e.kind() {
        io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied => InspectError::NotFound,
        io::ErrorKind::UnexpectedEof | io::ErrorKind::InvalidData => InspectError::Corrupt,
        _ => InspectError::Io(e.to_string()),
    }
}

fn map_rpm(e: rpm::Error) -> InspectError {
    match e {
        rpm::Error::Io(io) => map_io(io),
        // Bad magic, truncated header, missing tag: all mean "not a usable rpm".
        _ => InspectError::Corrupt,
    }
}

// ---------------------------------------------------------------- icon (F-08)

/// Rank of an icon path; lower is better. `None` = not an icon we want.
fn icon_rank(path: &str) -> Option<(IconKind, u8)> {
    let p = path.strip_prefix("./").unwrap_or(path);
    let p = p.strip_prefix('/').unwrap_or(p);
    let rest = p.strip_prefix("usr/share/")?;
    let (kind, rank) = if let Some(r) = rest.strip_prefix("icons/hicolor/") {
        let (dir, file) = r.split_once("/apps/")?;
        if file.contains('/') {
            return None;
        }
        let kind = if file.ends_with(".png") {
            IconKind::Png
        } else if file.ends_with(".svg") {
            IconKind::Svg
        } else {
            return None;
        };
        let rank = match (dir, kind) {
            ("256x256", IconKind::Png) => 0,
            ("scalable", IconKind::Svg) => 1,
            ("128x128", IconKind::Png) => 2,
            ("512x512", IconKind::Png) => 3,
            ("64x64", IconKind::Png) => 4,
            _ => return None,
        };
        (kind, rank)
    } else {
        let file = rest.strip_prefix("pixmaps/")?;
        if file.contains('/') || !file.ends_with(".png") {
            return None;
        }
        (IconKind::Png, 5)
    };
    Some((kind, rank))
}

fn file_stem(path: &str) -> &str {
    let file = path.rsplit('/').next().unwrap_or(path);
    file.rsplit_once('.').map_or(file, |(s, _)| s)
}

/// Stream the cpio payload, keep the best icon. Bounded by `PAYLOAD_SCAN_CAP`.
fn read_icon<R: BufRead>(reader: &mut R, pkg_name: &str) -> io::Result<Option<IconData>> {
    let input = open_payload(reader)?;
    scan_cpio(input.take(PAYLOAD_SCAN_CAP), pkg_name)
}

fn open_payload<'a, R: BufRead + 'a>(reader: &'a mut R) -> io::Result<Box<dyn Read + 'a>> {
    let magic = reader.fill_buf()?;
    let head = magic.get(..6).unwrap_or(magic);
    if head.starts_with(&[0x1f, 0x8b]) {
        Ok(Box::new(flate2::bufread::GzDecoder::new(reader)))
    } else if head.starts_with(&[0xfd, b'7', b'z', b'X', b'Z', 0]) {
        Ok(Box::new(liblzma::bufread::XzDecoder::new(reader)))
    } else if head.starts_with(&[0x28, 0xb5, 0x2f, 0xfd]) {
        Ok(Box::new(zstd::stream::read::Decoder::with_buffer(reader)?))
    } else if head.starts_with(b"07070") {
        Ok(Box::new(reader))
    } else {
        Err(io::ErrorKind::Unsupported.into())
    }
}

fn hex8(field: &[u8]) -> Option<u64> {
    u64::from_str_radix(std::str::from_utf8(field).ok()?, 16).ok()
}

fn skip<R: Read>(r: &mut R, n: u64) -> io::Result<()> {
    let copied = io::copy(&mut r.take(n), &mut io::sink())?;
    if copied == n {
        Ok(())
    } else {
        Err(io::ErrorKind::UnexpectedEof.into())
    }
}

/// Walk a `newc` cpio stream. "Stripped" cpio (`07070X`, >4 GiB payloads) is not supported.
fn scan_cpio<R: Read>(mut r: R, pkg_name: &str) -> io::Result<Option<IconData>> {
    let mut best: Option<((bool, u8), IconData)> = None;
    loop {
        let mut hdr = [0u8; 110];
        r.read_exact(&mut hdr)?;
        if &hdr[..6] != b"070701" && &hdr[..6] != b"070702" {
            return Ok(best.map(|(_, i)| i));
        }
        let field = |i: usize| hex8(&hdr[6 + i * 8..14 + i * 8]);
        let (Some(mode), Some(size), Some(name_len)) = (field(1), field(6), field(11)) else {
            return Ok(best.map(|(_, i)| i));
        };
        if name_len == 0 || name_len > CPIO_NAME_MAX {
            return Ok(best.map(|(_, i)| i));
        }
        let mut name = vec![0u8; name_len as usize];
        r.read_exact(&mut name)?;
        skip(&mut r, (4 - (110 + name_len) % 4) % 4)?;
        let name = String::from_utf8_lossy(&name[..name.len() - 1]).into_owned();
        if name == "TRAILER!!!" {
            return Ok(best.map(|(_, i)| i));
        }

        let pad = (4 - size % 4) % 4;
        let regular = mode & 0o170000 == 0o100000;
        let wanted = if regular && size > 0 && size <= ICON_MAX_BYTES {
            icon_rank(&name)
        } else {
            None
        };
        match wanted {
            Some((kind, rank)) => {
                let key = (file_stem(&name) != pkg_name, rank);
                let better = best.as_ref().is_none_or(|(k, _)| key < *k);
                if better {
                    // `size` is capped by ICON_MAX_BYTES above.
                    let mut bytes = vec![0u8; size as usize];
                    r.read_exact(&mut bytes)?;
                    best = Some((key, IconData { kind, bytes }));
                } else {
                    skip(&mut r, size)?;
                }
            }
            None => skip(&mut r, size)?,
        }
        skip(&mut r, pad)?;
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::Instant;

    use rpm::{CompressionWithLevel, FileOptions, PackageBuilder};

    pub(crate) fn scratch(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let dir = std::env::temp_dir().join(format!("s2b-{nanos}-{name}"));
        std::fs::create_dir_all(&dir).expect("mkdir scratch");
        dir
    }

    /// A tiny valid PNG (1x1) so the icon bytes look real.
    const PNG: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0x0d, 0x49, 0x48, 0x44, 0x52, 0,
        0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 0x1f, 0x15, 0xc4, 0x89,
    ];

    pub(crate) fn build_rpm(
        name: &str,
        epoch: Option<u32>,
        arch: &str,
        compression: CompressionWithLevel,
        with_icon: bool,
        pad_bytes: usize,
    ) -> Vec<u8> {
        let mut b = PackageBuilder::new(name, "1.2.3", "MIT", arch, "A tiny test app");
        b.release("4.fc45")
            .using_config(rpm::BuildConfig::v4().compression(compression));
        if let Some(e) = epoch {
            b.epoch(e);
        }
        b.with_file_contents(
            b"#!/bin/sh\n".to_vec(),
            FileOptions::new(format!("/usr/bin/{name}")).mode(0o100755),
        )
        .expect("add bin");
        if with_icon {
            b.with_file_contents(
                PNG.to_vec(),
                FileOptions::new(format!("/usr/share/icons/hicolor/256x256/apps/{name}.png")),
            )
            .expect("add icon");
            b.with_file_contents(
                b"<svg/>".to_vec(),
                FileOptions::new(format!("/usr/share/icons/hicolor/scalable/apps/{name}.svg")),
            )
            .expect("add svg");
        }
        if pad_bytes > 0 {
            // Incompressible-ish filler placed after the icon (path sorts later).
            let filler: Vec<u8> = (0..pad_bytes).map(|i| (i * 31 % 251) as u8).collect();
            b.with_file_contents(
                filler,
                FileOptions::new(format!("/usr/share/zzz/{name}.dat")),
            )
            .expect("add filler");
        }
        let pkg = b.build().expect("build rpm");
        let mut out = Vec::new();
        pkg.write(&mut out).expect("write rpm");
        out
    }

    fn write(dir: &Path, file: &str, bytes: &[u8]) -> PackageFile {
        let path = dir.join(file);
        std::fs::write(&path, bytes).expect("write fixture");
        PackageFile {
            path,
            format: PackageFormat::Rpm,
            size_bytes: bytes.len() as u64,
        }
    }

    fn run(file: &PackageFile) -> Result<PackageMetadata, InspectError> {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("rt")
            .block_on(RpmInspector::new().inspect(file))
    }

    #[test]
    fn happy_path_all_compressions_with_icon() {
        let dir = scratch("rpm-happy");
        for (label, c) in [
            ("gz", CompressionWithLevel::Gzip(6)),
            ("xz", CompressionWithLevel::Xz(1)),
            ("zst", CompressionWithLevel::Zstd(3)),
            ("none", CompressionWithLevel::None),
        ] {
            let bytes = build_rpm("lumen-notes", Some(2), "x86_64", c, true, 0);
            let f = write(&dir, &format!("{label}.rpm"), &bytes);
            let m = run(&f).unwrap_or_else(|e| panic!("{label}: {e}"));
            assert_eq!(m.name, "lumen-notes");
            assert_eq!(m.display_name, "Lumen Notes");
            assert_eq!(m.version.as_str(), "2:1.2.3-4.fc45", "{label}");
            assert_eq!(m.arch, Arch::X86_64);
            assert_eq!(m.summary.as_deref(), Some("A tiny test app"));
            assert_eq!(m.format, PackageFormat::Rpm);
            assert_eq!(m.scope, InstallScope::System);
            let icon = m.icon.unwrap_or_else(|| panic!("{label}: no icon"));
            assert_eq!(icon.kind, IconKind::Png, "256 png beats svg ({label})");
            assert_eq!(icon.bytes, PNG);
        }
    }

    #[test]
    fn no_epoch_no_icon() {
        let dir = scratch("rpm-noicon");
        let bytes = build_rpm(
            "tool",
            None,
            "noarch",
            CompressionWithLevel::Gzip(6),
            false,
            0,
        );
        let m = run(&write(&dir, "t.rpm", &bytes)).expect("inspect");
        assert_eq!(m.version.as_str(), "1.2.3-4.fc45");
        assert_eq!(m.arch, Arch::Noarch);
        assert!(m.icon.is_none());
    }

    #[test]
    fn wrong_arch_is_reported_not_rejected() {
        let dir = scratch("rpm-arch");
        let bytes = build_rpm(
            "tool",
            None,
            "aarch64",
            CompressionWithLevel::Gzip(6),
            false,
            0,
        );
        let m = run(&write(&dir, "a.rpm", &bytes)).expect("inspect");
        assert_eq!(m.arch, Arch::Aarch64);
        assert!(!m.arch.runs_on(&Arch::X86_64));
    }

    #[test]
    fn html_renamed_to_rpm_is_corrupt() {
        let dir = scratch("rpm-html");
        let f = write(
            &dir,
            "page.rpm",
            b"<!DOCTYPE html><html><body>404</body></html>",
        );
        assert_eq!(run(&f), Err(InspectError::Corrupt));
    }

    #[test]
    fn truncated_and_empty_are_corrupt() {
        let dir = scratch("rpm-trunc");
        let bytes = build_rpm(
            "tool",
            None,
            "x86_64",
            CompressionWithLevel::Gzip(6),
            false,
            0,
        );
        for cut in [0usize, 10, 96, 200] {
            let f = write(&dir, &format!("cut{cut}.rpm"), &bytes[..cut]);
            assert_eq!(run(&f), Err(InspectError::Corrupt), "cut at {cut}");
        }
    }

    #[test]
    fn missing_file_is_not_found() {
        let f = PackageFile {
            path: PathBuf::from("/nonexistent/x.rpm"),
            format: PackageFormat::Rpm,
            size_bytes: 0,
        };
        assert_eq!(run(&f), Err(InspectError::NotFound));
    }

    #[test]
    fn broken_payload_still_gives_metadata() {
        let dir = scratch("rpm-badpayload");
        let mut bytes = build_rpm(
            "tool",
            None,
            "x86_64",
            CompressionWithLevel::Gzip(6),
            true,
            0,
        );
        let n = bytes.len();
        bytes.truncate(n - 20);
        let m = run(&write(&dir, "b.rpm", &bytes)).expect("headers are fine");
        assert!(m.icon.is_none());
        assert_eq!(m.name, "tool");
    }

    #[test]
    fn supports_only_rpm() {
        let i = RpmInspector::new();
        assert!(i.supports(PackageFormat::Rpm));
        assert!(!i.supports(PackageFormat::Deb));
        assert!(!i.supports(PackageFormat::AppImage));
    }

    #[test]
    fn display_name_table() {
        for (raw, want) in [
            ("lumen-notes", "Lumen Notes"),
            ("gnome_tweaks", "Gnome Tweaks"),
            ("firefox", "Firefox"),
            ("libfoo2.1", "Libfoo2 1"),
            ("", ""),
        ] {
            assert_eq!(display_name_from(raw), want);
        }
    }

    #[test]
    fn icon_rank_table() {
        assert_eq!(
            icon_rank("./usr/share/icons/hicolor/256x256/apps/a.png"),
            Some((IconKind::Png, 0))
        );
        assert_eq!(
            icon_rank("./usr/share/icons/hicolor/scalable/apps/a.svg"),
            Some((IconKind::Svg, 1))
        );
        assert_eq!(
            icon_rank("./usr/share/icons/hicolor/256x256/apps/a.svg"),
            None
        );
        assert_eq!(
            icon_rank("./usr/share/icons/hicolor/48x48/apps/a.png"),
            None
        );
        assert_eq!(icon_rank("./usr/bin/a"), None);
        assert_eq!(
            icon_rank("./usr/share/icons/hicolor/256x256/apps/x/a.png"),
            None
        );
    }

    /// F-08 cost note. Run: `cargo test -p installer-infrastructure icon_cost -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn icon_cost_report() {
        let dir = scratch("rpm-cost");
        for (label, c) in [
            ("gzip", CompressionWithLevel::Gzip(6)),
            ("zstd", CompressionWithLevel::Zstd(3)),
            ("xz", CompressionWithLevel::Xz(1)),
        ] {
            let bytes = build_rpm("big", None, "x86_64", c, true, 32 * 1024 * 1024);
            let f = write(&dir, &format!("{label}.rpm"), &bytes);
            let t = Instant::now();
            RpmInspector::inspect_path(&f.path, false).expect("headers");
            let headers = t.elapsed();
            let t = Instant::now();
            let m = RpmInspector::inspect_path(&f.path, true).expect("full");
            println!(
                "{label}: file {} KiB, headers only {headers:?}, with icon scan {:?}, icon {}",
                bytes.len() / 1024,
                t.elapsed(),
                m.icon.is_some()
            );
        }
    }

    /// Regenerates the small committed fixture. `-- --ignored regenerate_rpm_fixture`
    #[test]
    #[ignore]
    fn regenerate_rpm_fixture() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rpm");
        std::fs::create_dir_all(&dir).expect("mkdir");
        let bytes = build_rpm(
            "hello-world",
            None,
            "x86_64",
            CompressionWithLevel::Gzip(9),
            true,
            0,
        );
        std::fs::write(dir.join("hello-world-1.2.3-4.fc45.x86_64.rpm"), bytes).expect("write");
        std::fs::write(
            dir.join("not-a-package.rpm"),
            b"<!DOCTYPE html><html></html>",
        )
        .expect("write");
    }

    #[test]
    fn committed_fixture_parses() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rpm");
        let path = dir.join("hello-world-1.2.3-4.fc45.x86_64.rpm");
        let m = RpmInspector::inspect_path(&path, true).expect("fixture");
        assert_eq!(m.display_name, "Hello World");
        assert!(m.icon.is_some());
        let bad = dir.join("not-a-package.rpm");
        assert_eq!(
            RpmInspector::inspect_path(&bad, true),
            Err(InspectError::Corrupt)
        );
    }
}
