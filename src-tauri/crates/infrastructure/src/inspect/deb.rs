//! `.deb` inspector (F-09). ar -> `control.tar.{gz,xz,zst}` -> `control` fields.
//! Reads bytes only; never runs the package (ADR-0005).

use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use installer_application::ports::PackageInspector;
use installer_domain::{
    Arch, InspectError, InstallScope, PackageFile, PackageFormat, PackageMetadata,
    UnsupportedReason, Version,
};

use super::rpm::{display_name_from, map_io};

const AR_MAGIC: &[u8; 8] = b"!<arch>\n";
const AR_HEADER_LEN: usize = 60;
/// Largest compressed `control.tar.*` we read.
const CONTROL_ARCHIVE_MAX: u64 = 8 * 1024 * 1024;
/// Largest decompressed `control` file we read.
const CONTROL_FILE_MAX: u64 = 1024 * 1024;
/// Members to look at before giving up (a real .deb has three).
const AR_MEMBERS_MAX: usize = 16;

/// Inspects `.deb` files by reading only the control archive.
#[derive(Debug, Default, Clone, Copy)]
pub struct DebInspector;

impl DebInspector {
    pub fn new() -> Self {
        Self
    }

    /// Blocking worker behind [`PackageInspector::inspect`].
    pub fn inspect_path(path: &Path) -> Result<PackageMetadata, InspectError> {
        let mut reader = BufReader::new(File::open(path).map_err(map_io)?);
        let (member, bytes) = read_control_member(&mut reader)?;
        let control = decompress_control(&member, bytes)?;
        let fields = parse_control(&control);

        let get = |k: &str| {
            fields
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(k))
                .map(|(_, v)| v.as_str())
        };
        let name = get("Package")
            .filter(|s| !s.is_empty())
            .ok_or(InspectError::Corrupt)?;
        let version = get("Version")
            .filter(|s| !s.is_empty())
            .ok_or(InspectError::Corrupt)?;
        let arch = get("Architecture")
            .filter(|s| !s.is_empty())
            .ok_or(InspectError::Corrupt)?;
        let summary = get("Description")
            .and_then(|d| d.lines().next())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned);

        Ok(PackageMetadata {
            format: PackageFormat::Deb,
            display_name: display_name_from(name),
            name: name.to_owned(),
            version: Version::new(version),
            arch: Arch::parse(arch),
            summary,
            icon: None,
            scope: InstallScope::for_format(PackageFormat::Deb),
            needs_fuse2: false,
        })
    }
}

impl PackageInspector for DebInspector {
    fn supports(&self, format: PackageFormat) -> bool {
        format == PackageFormat::Deb
    }

    async fn inspect(&self, file: &PackageFile) -> Result<PackageMetadata, InspectError> {
        if file.format != PackageFormat::Deb {
            return Err(InspectError::Unsupported(UnsupportedReason::UnknownFormat));
        }
        let path = file.path.clone();
        tokio::task::spawn_blocking(move || Self::inspect_path(&path))
            .await
            .map_err(|e| InspectError::Io(e.to_string()))?
    }
}

/// Walk ar members, return the name and raw bytes of `control.tar*`.
fn read_control_member<R: Read + Seek>(r: &mut R) -> Result<(String, Vec<u8>), InspectError> {
    let mut magic = [0u8; 8];
    r.read_exact(&mut magic).map_err(map_io)?;
    if &magic != AR_MAGIC {
        return Err(InspectError::Corrupt);
    }
    for index in 0..AR_MEMBERS_MAX {
        let mut h = [0u8; AR_HEADER_LEN];
        match r.read_exact(&mut h) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                return Err(InspectError::Corrupt);
            }
            Err(e) => return Err(map_io(e)),
        }
        if &h[58..60] != b"`\n" {
            return Err(InspectError::Corrupt);
        }
        let name = String::from_utf8_lossy(&h[..16])
            .trim()
            .trim_end_matches('/')
            .to_owned();
        let size: u64 = std::str::from_utf8(&h[48..58])
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .ok_or(InspectError::Corrupt)?;
        if index == 0 && name != "debian-binary" {
            return Err(InspectError::Corrupt);
        }
        if name.starts_with("control.tar") {
            if size > CONTROL_ARCHIVE_MAX {
                return Err(InspectError::Corrupt);
            }
            let mut buf = vec![0u8; size as usize];
            r.read_exact(&mut buf).map_err(map_io)?;
            return Ok((name, buf));
        }
        // Skip data plus the pad byte that keeps members even-aligned.
        let skip = i64::try_from(size + (size % 2)).map_err(|_| InspectError::Corrupt)?;
        r.seek(SeekFrom::Current(skip)).map_err(map_io)?;
    }
    Err(InspectError::Corrupt)
}

fn decompress_control(member: &str, bytes: Vec<u8>) -> Result<Vec<u8>, InspectError> {
    let reader: Box<dyn Read + '_> = match member {
        "control.tar.gz" => Box::new(flate2::read::GzDecoder::new(bytes.as_slice())),
        "control.tar.xz" => Box::new(liblzma::read::XzDecoder::new(bytes.as_slice())),
        "control.tar.zst" => {
            Box::new(zstd::stream::read::Decoder::new(bytes.as_slice()).map_err(map_io)?)
        }
        "control.tar" => Box::new(bytes.as_slice()),
        // bzip2 and friends: valid deb, but we do not read it.
        _ => return Err(InspectError::Unsupported(UnsupportedReason::UnknownFormat)),
    };
    find_in_tar(reader, "control")
}

fn octal(field: &[u8]) -> Option<u64> {
    let s = std::str::from_utf8(field).ok()?;
    let s = s.trim_matches(|c: char| c == '\0' || c == ' ');
    if s.is_empty() {
        Some(0)
    } else {
        u64::from_str_radix(s, 8).ok()
    }
}

/// Minimal ustar walk: find a regular file named `wanted` (with or without `./`).
fn find_in_tar<R: Read>(mut r: R, wanted: &str) -> Result<Vec<u8>, InspectError> {
    loop {
        let mut block = [0u8; 512];
        r.read_exact(&mut block).map_err(map_io)?;
        if block.iter().all(|b| *b == 0) {
            return Err(InspectError::Corrupt); // end of archive, no control file
        }
        let name_end = block[..100].iter().position(|b| *b == 0).unwrap_or(100);
        let name = String::from_utf8_lossy(&block[..name_end]);
        let size = octal(&block[124..136]).ok_or(InspectError::Corrupt)?;
        let typeflag = block[156];
        let padded = size.div_ceil(512) * 512;
        let is_file = typeflag == b'0' || typeflag == 0;
        if is_file && name.trim_start_matches("./") == wanted {
            if size > CONTROL_FILE_MAX {
                return Err(InspectError::Corrupt);
            }
            let mut data = vec![0u8; size as usize];
            r.read_exact(&mut data).map_err(map_io)?;
            return Ok(data);
        }
        let copied = io::copy(&mut r.by_ref().take(padded), &mut io::sink()).map_err(map_io)?;
        if copied != padded {
            return Err(InspectError::Corrupt);
        }
    }
}

/// Parse RFC822-style fields. Continuation lines (leading space/tab) join with `\n`.
fn parse_control(bytes: &[u8]) -> Vec<(String, String)> {
    let text = String::from_utf8_lossy(bytes);
    let mut out: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        if line.starts_with([' ', '\t']) {
            if let Some((_, v)) = out.last_mut() {
                v.push('\n');
                v.push_str(line.trim());
            }
        } else if let Some((k, v)) = line.split_once(':') {
            out.push((k.trim().to_owned(), v.trim().to_owned()));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inspect::rpm::tests::scratch;
    use std::io::Write;

    fn tar_with_control(control: &str) -> Vec<u8> {
        fn entry(name: &str, typeflag: u8, data: &[u8]) -> Vec<u8> {
            let mut h = [0u8; 512];
            h[..name.len()].copy_from_slice(name.as_bytes());
            h[100..107].copy_from_slice(b"0000644");
            let size = format!("{:011o}", data.len());
            h[124..135].copy_from_slice(size.as_bytes());
            h[156] = typeflag;
            h[257..262].copy_from_slice(b"ustar");
            let mut out = h.to_vec();
            out.extend_from_slice(data);
            out.resize(512 + data.len().div_ceil(512) * 512, 0);
            out
        }
        let mut t = entry("./", b'5', b"");
        t.extend(entry(
            "./md5sums",
            b'0',
            b"d41d8cd98f00b204e9800998ecf8427e  usr/bin/x\n",
        ));
        t.extend(entry("./control", b'0', control.as_bytes()));
        t.extend(vec![0u8; 1024]);
        t
    }

    fn ar(members: &[(&str, Vec<u8>)]) -> Vec<u8> {
        let mut out = AR_MAGIC.to_vec();
        for (name, data) in members {
            out.extend(
                format!(
                    "{name:<16}{:<12}{:<6}{:<6}{:<8}{:<10}`\n",
                    0,
                    0,
                    0,
                    "100644",
                    data.len()
                )
                .bytes(),
            );
            out.extend(data);
            if data.len() % 2 == 1 {
                out.push(b'\n');
            }
        }
        out
    }

    pub(crate) fn build_deb(control: &str, ext: &str) -> Vec<u8> {
        let tar = tar_with_control(control);
        let packed = match ext {
            "gz" => {
                let mut e =
                    flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
                e.write_all(&tar).expect("gz");
                e.finish().expect("gz")
            }
            "xz" => {
                let mut e = liblzma::write::XzEncoder::new(Vec::new(), 1);
                e.write_all(&tar).expect("xz");
                e.finish().expect("xz")
            }
            "zst" => zstd::stream::encode_all(tar.as_slice(), 3).expect("zst"),
            _ => tar,
        };
        let ctl_name = if ext.is_empty() {
            "control.tar".to_owned()
        } else {
            format!("control.tar.{ext}")
        };
        ar(&[
            ("debian-binary", b"2.0\n".to_vec()),
            (&ctl_name, packed),
            ("data.tar.xz", vec![1, 2, 3]), // odd length: exercises ar padding
        ])
    }

    const CONTROL: &str = "Package: lumen-notes\nVersion: 1:2.3.4-1ubuntu1\nArchitecture: amd64\nMaintainer: A <a@example.com>\nDescription: Tiny notes app\n A longer text that\n spans lines.\n";

    fn write(dir: &Path, name: &str, bytes: &[u8]) -> PackageFile {
        let path = dir.join(name);
        std::fs::write(&path, bytes).expect("write");
        PackageFile {
            path,
            format: PackageFormat::Deb,
            size_bytes: bytes.len() as u64,
        }
    }

    fn run(f: &PackageFile) -> Result<PackageMetadata, InspectError> {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("rt")
            .block_on(DebInspector::new().inspect(f))
    }

    #[test]
    fn happy_path_every_compression() {
        let dir = scratch("deb-happy");
        for ext in ["gz", "xz", "zst", ""] {
            let f = write(&dir, &format!("a{ext}.deb"), &build_deb(CONTROL, ext));
            let m = run(&f).unwrap_or_else(|e| panic!("{ext}: {e}"));
            assert_eq!(m.name, "lumen-notes");
            assert_eq!(m.display_name, "Lumen Notes");
            assert_eq!(m.version.as_str(), "1:2.3.4-1ubuntu1");
            assert_eq!(m.arch, Arch::X86_64);
            assert_eq!(m.summary.as_deref(), Some("Tiny notes app"));
            assert_eq!(m.format, PackageFormat::Deb);
            assert!(m.icon.is_none());
        }
    }

    #[test]
    fn wrong_arch_is_reported() {
        let dir = scratch("deb-arch");
        let c = CONTROL.replace("amd64", "arm64");
        let m = run(&write(&dir, "a.deb", &build_deb(&c, "gz"))).expect("inspect");
        assert_eq!(m.arch, Arch::Aarch64);
        assert!(!m.arch.runs_on(&Arch::X86_64));
    }

    #[test]
    fn arch_all_is_noarch() {
        let dir = scratch("deb-all");
        let c = CONTROL.replace("amd64", "all");
        let m = run(&write(&dir, "a.deb", &build_deb(&c, "xz"))).expect("inspect");
        assert_eq!(m.arch, Arch::Noarch);
    }

    #[test]
    fn html_renamed_to_deb_is_corrupt() {
        let dir = scratch("deb-html");
        let f = write(
            &dir,
            "p.deb",
            b"<!DOCTYPE html><html><body>404 not found</body></html>",
        );
        assert_eq!(run(&f), Err(InspectError::Corrupt));
    }

    #[test]
    fn truncated_and_missing_fields_are_corrupt() {
        let dir = scratch("deb-bad");
        let bytes = build_deb(CONTROL, "gz");
        for cut in [0usize, 7, 8, 40, 80, 100] {
            let f = write(&dir, &format!("c{cut}.deb"), &bytes[..cut]);
            assert_eq!(run(&f), Err(InspectError::Corrupt), "cut {cut}");
        }
        let no_pkg = "Version: 1\nArchitecture: amd64\n";
        assert_eq!(
            run(&write(&dir, "n.deb", &build_deb(no_pkg, "gz"))),
            Err(InspectError::Corrupt)
        );
    }

    #[test]
    fn missing_file_is_not_found() {
        let f = PackageFile {
            path: "/nonexistent/x.deb".into(),
            format: PackageFormat::Deb,
            size_bytes: 0,
        };
        assert_eq!(run(&f), Err(InspectError::NotFound));
    }

    #[test]
    fn parse_control_continuations() {
        let f = parse_control(CONTROL.as_bytes());
        let d = f.iter().find(|(k, _)| k == "Description").expect("desc");
        assert_eq!(d.1, "Tiny notes app\nA longer text that\nspans lines.");
    }

    #[test]
    fn supports_only_deb() {
        let i = DebInspector::new();
        assert!(i.supports(PackageFormat::Deb));
        assert!(!i.supports(PackageFormat::Rpm));
    }

    /// Regenerates the small committed fixture. `-- --ignored regenerate_deb_fixture`
    #[test]
    #[ignore]
    fn regenerate_deb_fixture() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/deb");
        std::fs::create_dir_all(&dir).expect("mkdir");
        std::fs::write(dir.join("hello-world_1.2.3-1_amd64.deb"), build_deb(
            "Package: hello-world\nVersion: 1.2.3-1\nArchitecture: amd64\nDescription: A tiny test app\n",
            "gz",
        )).expect("write");
        std::fs::write(
            dir.join("not-a-package.deb"),
            b"<!DOCTYPE html><html></html>",
        )
        .expect("write");
    }

    #[test]
    fn committed_fixture_parses() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/deb");
        let m = DebInspector::inspect_path(&dir.join("hello-world_1.2.3-1_amd64.deb"))
            .expect("fixture");
        assert_eq!(m.display_name, "Hello World");
        assert_eq!(
            DebInspector::inspect_path(&dir.join("not-a-package.deb")),
            Err(InspectError::Corrupt)
        );
    }
}
