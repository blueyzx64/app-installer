//! AppImage (type 2) inspector (F-04, ADR-0005).
//!
//! Reads the ELF header to find the SquashFS image, opens it with `backhand`, and pulls
//! the root `*.desktop` and the icon out. The file is only ever read, never run.
//!
//! `needs_fuse2` heuristic: the runtime (bytes before the SquashFS image) needs
//! `libfuse.so.2` when it is dynamically linked (has a `PT_INTERP` program header) and
//! mentions the string `libfuse.so.2` (its `DT_NEEDED` entry lives in `.dynstr`). A static
//! runtime (no `PT_INTERP`, the modern type2-runtime) never needs it. A dynamic runtime
//! without the string is assumed to want libfuse3 or nothing, so we do not flag it.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};

use backhand::{FilesystemReader, InnerNode, Node, SquashfsFileReader};
use installer_application::ports::PackageInspector;
use installer_domain::{
    AppId, IconData, IconKind, InspectError, InstallScope, PackageFile, PackageFormat,
    PackageMetadata, Version,
};

use super::elf::{self, ELF_HEADER_LEN, ElfHeader};

/// Largest file we read out of the image (desktop entry, icon).
pub const MAX_IMAGE_FILE_BYTES: u64 = 2 * 1024 * 1024;
/// Symlink hops we follow inside the image.
pub const MAX_SYMLINK_DEPTH: usize = 8;
/// Largest runtime we scan for the fuse2 heuristic.
const MAX_RUNTIME_BYTES: u64 = 8 * 1024 * 1024;
/// Icons wider than this are scaled down.
const MAX_ICON_PX: u32 = 256;

#[derive(Debug, Default, Clone, Copy)]
pub struct AppImageInspector;

impl PackageInspector for AppImageInspector {
    fn supports(&self, format: PackageFormat) -> bool {
        format == PackageFormat::AppImage
    }

    async fn inspect(&self, file: &PackageFile) -> Result<PackageMetadata, InspectError> {
        let path = file.path.clone();
        tokio::task::spawn_blocking(move || inspect_path(&path))
            .await
            .map_err(|e| InspectError::Io(e.to_string()))?
    }
}

fn io_err(e: std::io::Error) -> InspectError {
    match e.kind() {
        std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied => {
            InspectError::NotFound
        }
        _ => InspectError::Io(e.to_string()),
    }
}

/// Blocking inspection of one AppImage file.
pub fn inspect_path(path: &Path) -> Result<PackageMetadata, InspectError> {
    let mut f = File::open(path).map_err(io_err)?;
    let len = f.metadata().map_err(io_err)?.len();

    let mut head = vec![0u8; ELF_HEADER_LEN.min(usize::try_from(len).unwrap_or(usize::MAX))];
    f.read_exact(&mut head).map_err(|_| InspectError::Corrupt)?;
    let header = ElfHeader::parse(&head)?;
    elf::require_type_two(&header)?;
    let offset = header.squashfs_offset().ok_or(InspectError::Corrupt)?;
    if offset.checked_add(4).is_none_or(|end| end > len) {
        return Err(InspectError::Corrupt);
    }

    let mut magic = [0u8; 8];
    f.seek(SeekFrom::Start(offset)).map_err(io_err)?;
    let n = f.read(&mut magic).map_err(io_err)?;
    elf::classify_image(&magic[..n])?;

    // Runtime bytes for the fuse2 heuristic.
    f.seek(SeekFrom::Start(0)).map_err(io_err)?;
    let mut runtime = Vec::new();
    f.by_ref()
        .take(offset.min(MAX_RUNTIME_BYTES))
        .read_to_end(&mut runtime)
        .map_err(io_err)?;
    let needs_fuse2 = header.is_dynamic(&runtime) && contains(&runtime, b"libfuse.so.2");

    let fs = FilesystemReader::from_reader_with_offset(BufReader::new(f), offset)
        .map_err(|_| InspectError::Corrupt)?;
    let image = Image::new(&fs);

    let (desktop_text, _) = image.root_desktop_entry().ok_or(InspectError::Corrupt)?;
    let entry = parse_desktop_entry(&desktop_text);
    let file_stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();

    let display_name = entry
        .get("Name")
        .filter(|s| !s.trim().is_empty())
        .map_or_else(|| name_from_file(&file_stem), |s| s.trim().to_owned());
    let version = entry
        .get("X-AppImage-Version")
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .or_else(|| version_from_file(&file_stem))
        .unwrap_or_else(|| "0".to_owned());
    let summary = entry
        .get("Comment")
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty());
    let icon = image.icon(entry.get("Icon").map(String::as_str));

    Ok(PackageMetadata {
        format: PackageFormat::AppImage,
        name: AppId::from_name(&display_name).as_str().to_owned(),
        display_name,
        version: Version::new(version),
        arch: header.arch(),
        summary,
        icon,
        scope: InstallScope::User,
        needs_fuse2,
    })
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

/// "Lumen-Notes-1.2.3-x86_64" → "Lumen Notes" (name part before the first digit-led token).
fn name_from_file(stem: &str) -> String {
    let parts: Vec<&str> = stem
        .split(['-', '_', ' '])
        .take_while(|p| !p.starts_with(|c: char| c.is_ascii_digit()))
        .collect();
    let joined = parts.join(" ");
    if joined.trim().is_empty() {
        stem.to_owned()
    } else {
        joined
    }
}

/// First token starting with a digit, e.g. "1.2.3".
fn version_from_file(stem: &str) -> Option<String> {
    stem.split(['-', '_', ' '])
        .find(|p| p.starts_with(|c: char| c.is_ascii_digit()) && p.contains('.'))
        .map(str::to_owned)
}

/// Unlocalised `key=value` pairs of the `[Desktop Entry]` group. Escapes are not decoded
/// beyond `\\`, `\n`, `\s`, `\t` (the values we show are plain text).
pub fn parse_desktop_entry(text: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let mut in_group = false;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            in_group = line == "[Desktop Entry]";
            continue;
        }
        if !in_group {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let k = k.trim();
            if k.contains('[') {
                continue; // localised key
            }
            out.entry(k.to_owned())
                .or_insert_with(|| unescape(v.trim()));
        }
    }
    out
}

fn unescape(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let mut chars = v.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('s') => out.push(' '),
            Some('t') => out.push('\t'),
            Some('\\') => out.push('\\'),
            Some(o) => {
                out.push('\\');
                out.push(o);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Path lookup inside the SquashFS image. Symlinks are followed only inside the image.
struct Image<'a, 'b> {
    fs: &'a FilesystemReader<'b>,
    nodes: HashMap<PathBuf, &'a Node<SquashfsFileReader>>,
}

impl<'a, 'b> Image<'a, 'b> {
    fn new(fs: &'a FilesystemReader<'b>) -> Self {
        let nodes = fs.files().map(|n| (n.fullpath.clone(), n)).collect();
        Image { fs, nodes }
    }

    /// Resolve `path` to a regular file, following at most `MAX_SYMLINK_DEPTH` links.
    /// Targets are clamped to the image root, so `../../etc/passwd` stays inside.
    fn resolve(&self, path: &Path) -> Option<&'a SquashfsFileReader> {
        let mut cur = normalize(Path::new("/"), path);
        for _ in 0..=MAX_SYMLINK_DEPTH {
            let node = self.nodes.get(&cur)?;
            match &node.inner {
                InnerNode::File(f) => return Some(f),
                InnerNode::Symlink(s) => {
                    let parent = cur.parent().unwrap_or(Path::new("/")).to_path_buf();
                    cur = normalize(&parent, &s.link);
                }
                _ => return None,
            }
        }
        None
    }

    fn read(&self, file: &SquashfsFileReader) -> Option<Vec<u8>> {
        let mut buf = Vec::new();
        self.fs
            .file(file)
            .reader()
            .take(MAX_IMAGE_FILE_BYTES + 1)
            .read_to_end(&mut buf)
            .ok()?;
        (buf.len() as u64 <= MAX_IMAGE_FILE_BYTES).then_some(buf)
    }

    fn read_path(&self, path: &Path) -> Option<Vec<u8>> {
        self.read(self.resolve(path)?)
    }

    /// The first root-level `*.desktop` (sorted by name so the choice is stable).
    fn root_desktop_entry(&self) -> Option<(String, PathBuf)> {
        let mut names: Vec<&PathBuf> = self
            .nodes
            .keys()
            .filter(|p| {
                p.parent() == Some(Path::new("/")) && p.extension().is_some_and(|e| e == "desktop")
            })
            .collect();
        names.sort();
        for p in names {
            if let Some(bytes) = self.read_path(p) {
                return Some((String::from_utf8_lossy(&bytes).into_owned(), p.clone()));
            }
        }
        None
    }

    /// `.DirIcon`, then `<Icon>.png|svg` at the root.
    fn icon(&self, icon_name: Option<&str>) -> Option<IconData> {
        let mut candidates = vec![PathBuf::from("/.DirIcon")];
        if let Some(n) = icon_name.filter(|n| !n.is_empty() && !n.contains('/')) {
            candidates.push(PathBuf::from(format!("/{n}.png")));
            candidates.push(PathBuf::from(format!("/{n}.svg")));
        }
        candidates
            .iter()
            .filter_map(|p| self.read_path(p))
            .find_map(sniff_icon)
    }
}

/// Join `rel` onto `base` lexically; `..` never climbs above `/`.
fn normalize(base: &Path, rel: &Path) -> PathBuf {
    let mut parts: Vec<std::ffi::OsString> = if rel.is_absolute() {
        Vec::new()
    } else {
        base.components()
            .filter_map(|c| match c {
                Component::Normal(s) => Some(s.to_owned()),
                _ => None,
            })
            .collect()
    };
    for c in rel.components() {
        match c {
            Component::Normal(s) => parts.push(s.to_owned()),
            Component::ParentDir => {
                parts.pop();
            }
            _ => {}
        }
    }
    let mut out = PathBuf::from("/");
    out.extend(parts);
    out
}

/// Accept PNG (scaled to at most 256 px) or SVG; anything else is no icon.
fn sniff_icon(bytes: Vec<u8>) -> Option<IconData> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some(IconData {
            kind: IconKind::Png,
            bytes: shrink_png(bytes),
        });
    }
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]).into_owned();
    if head.contains("<svg") || (head.trim_start().starts_with("<?xml") && head.contains("svg")) {
        return Some(IconData {
            kind: IconKind::Svg,
            bytes,
        });
    }
    None
}

fn shrink_png(bytes: Vec<u8>) -> Vec<u8> {
    let Ok(img) = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png) else {
        return bytes;
    };
    if img.width() <= MAX_ICON_PX && img.height() <= MAX_ICON_PX {
        return bytes;
    }
    let small = img.resize(
        MAX_ICON_PX,
        MAX_ICON_PX,
        image::imageops::FilterType::Lanczos3,
    );
    let mut out = std::io::Cursor::new(Vec::new());
    match small.write_to(&mut out, image::ImageFormat::Png) {
        Ok(()) => out.into_inner(),
        Err(_) => bytes,
    }
}

/// Test helper: build a tiny type-2 AppImage (ELF stub + SquashFS) in memory.
#[cfg(test)]
pub(crate) mod testkit {
    use std::io::Cursor;

    use backhand::{FilesystemWriter, NodeHeader};

    use super::super::elf::testutil::elf_stub;

    pub enum Entry<'a> {
        File(&'a str, Vec<u8>),
        Link(&'a str, &'a str),
    }

    pub struct Spec<'a> {
        pub machine: u16,
        pub ai_type: u8,
        pub dynamic: bool,
        pub entries: Vec<Entry<'a>>,
    }

    impl Default for Spec<'_> {
        fn default() -> Self {
            Spec {
                machine: 62,
                ai_type: 2,
                dynamic: false,
                entries: vec![],
            }
        }
    }

    pub fn tiny_png(side: u32) -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(side, side, image::Rgba([200, 60, 60, 255]));
        let mut out = Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    pub fn desktop(name: &str, icon: &str, version: Option<&str>) -> Vec<u8> {
        let mut s = format!(
            "[Desktop Entry]\nType=Application\nName={name}\nName[de]=Falsch\nComment=A tiny test app\nIcon={icon}\nExec=AppRun\nCategories=Utility;\n"
        );
        if let Some(v) = version {
            s.push_str(&format!("X-AppImage-Version={v}\n"));
        }
        s.into_bytes()
    }

    pub fn build(spec: Spec) -> Vec<u8> {
        let mut w = FilesystemWriter::default();
        let hdr = NodeHeader::new(0o644, 0, 0, 0);
        for e in spec.entries {
            match e {
                Entry::File(p, data) => w.push_file(Cursor::new(data), p, hdr).unwrap(),
                Entry::Link(p, target) => w.push_symlink(target, p, hdr).unwrap(),
            }
        }
        let mut sq = Cursor::new(Vec::new());
        w.write(&mut sq).unwrap();
        let mut out = elf_stub(true, false, spec.machine, spec.ai_type, spec.dynamic);
        out.extend_from_slice(&sq.into_inner());
        out
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use installer_domain::UnsupportedReason;

    use super::testkit::{Entry, Spec, build, desktop, tiny_png};
    use super::*;

    fn write_tmp(bytes: &[u8]) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("Lumen-Notes-1.4.2-x86_64.AppImage");
        fs::write(&p, bytes).unwrap();
        (dir, p)
    }

    fn basic() -> Spec<'static> {
        Spec {
            entries: vec![
                Entry::File(
                    "/lumen.desktop",
                    desktop("Lumen Notes", "lumen", Some("1.4.2")),
                ),
                Entry::File("/lumen.png", tiny_png(32)),
                Entry::Link("/.DirIcon", "lumen.png"),
            ],
            ..Spec::default()
        }
    }

    #[test]
    fn reads_name_version_icon_arch() {
        let (_d, p) = write_tmp(&build(basic()));
        let m = inspect_path(&p).unwrap();
        assert_eq!(m.display_name, "Lumen Notes");
        assert_eq!(m.name, "lumen-notes");
        assert_eq!(m.version.as_str(), "1.4.2");
        assert_eq!(m.summary.as_deref(), Some("A tiny test app"));
        assert_eq!(m.arch, installer_domain::Arch::X86_64);
        assert_eq!(m.scope, InstallScope::User);
        assert!(!m.needs_fuse2);
        let icon = m.icon.unwrap();
        assert_eq!(icon.kind, IconKind::Png);
    }

    #[test]
    fn aarch64_machine() {
        let mut s = basic();
        s.machine = 183;
        let (_d, p) = write_tmp(&build(s));
        assert_eq!(
            inspect_path(&p).unwrap().arch,
            installer_domain::Arch::Aarch64
        );
    }

    #[test]
    fn version_falls_back_to_file_name() {
        let s = Spec {
            entries: vec![Entry::File(
                "/a.desktop",
                desktop("Lumen Notes", "lumen", None),
            )],
            ..Spec::default()
        };
        let (_d, p) = write_tmp(&build(s));
        let m = inspect_path(&p).unwrap();
        assert_eq!(m.version.as_str(), "1.4.2");
        assert!(m.icon.is_none());
    }

    #[test]
    fn icon_by_name_when_no_diricon() {
        let s = Spec {
            entries: vec![
                Entry::File("/a.desktop", desktop("A", "app", None)),
                Entry::File(
                    "/app.svg",
                    b"<svg xmlns='http://www.w3.org/2000/svg'/>".to_vec(),
                ),
            ],
            ..Spec::default()
        };
        let (_d, p) = write_tmp(&build(s));
        assert_eq!(inspect_path(&p).unwrap().icon.unwrap().kind, IconKind::Svg);
    }

    #[test]
    fn big_png_is_scaled_down() {
        let s = Spec {
            entries: vec![
                Entry::File("/a.desktop", desktop("A", "app", None)),
                Entry::File("/.DirIcon", tiny_png(512)),
            ],
            ..Spec::default()
        };
        let (_d, p) = write_tmp(&build(s));
        let icon = inspect_path(&p).unwrap().icon.unwrap();
        let img = image::load_from_memory(&icon.bytes).unwrap();
        assert_eq!(img.width(), 256);
    }

    #[test]
    fn symlink_cannot_escape_image_and_depth_is_bounded() {
        let s = Spec {
            entries: vec![
                Entry::File("/a.desktop", desktop("A", "app", None)),
                Entry::Link("/.DirIcon", "../../../etc/passwd"),
                Entry::Link("/l1", "/l2"),
                Entry::Link("/l2", "/l1"),
            ],
            ..Spec::default()
        };
        let (_d, p) = write_tmp(&build(s));
        assert!(inspect_path(&p).unwrap().icon.is_none());
        // The loop resolves to nothing instead of hanging.
        let bytes = build(Spec {
            entries: vec![
                Entry::File("/a.desktop", desktop("A", "l1", None)),
                Entry::Link("/l1", "/l2"),
                Entry::Link("/l2", "/l1"),
            ],
            ..Spec::default()
        });
        let (_d2, p2) = write_tmp(&bytes);
        assert!(inspect_path(&p2).unwrap().icon.is_none());
    }

    #[test]
    fn normalize_table() {
        let rows = [
            ("/", "a/b", "/a/b"),
            ("/x", "../../a", "/a"),
            ("/x", "/a/../b", "/b"),
            ("/x/y", "./z", "/x/y/z"),
        ];
        for (base, rel, want) in rows {
            assert_eq!(
                normalize(Path::new(base), Path::new(rel)),
                PathBuf::from(want)
            );
        }
    }

    #[test]
    fn unsupported_and_corrupt() {
        // Type 1
        let (_d, p) = write_tmp(&build(Spec {
            ai_type: 1,
            ..basic()
        }));
        assert_eq!(
            inspect_path(&p),
            Err(InspectError::Unsupported(UnsupportedReason::AppImageType1))
        );
        // DwarFS
        let mut b = build(basic());
        let off = elf::ElfHeader::parse(&b)
            .unwrap()
            .squashfs_offset()
            .unwrap() as usize;
        b[off..off + 6].copy_from_slice(b"DWARFS");
        let (_d, p) = write_tmp(&b);
        assert_eq!(
            inspect_path(&p),
            Err(InspectError::Unsupported(UnsupportedReason::Dwarfs))
        );
        // Truncated squashfs and plain junk
        let b = build(basic());
        let off = elf::ElfHeader::parse(&b)
            .unwrap()
            .squashfs_offset()
            .unwrap() as usize;
        let (_d, p) = write_tmp(&b[..off + 100]);
        assert_eq!(inspect_path(&p), Err(InspectError::Corrupt));
        let (_d, p) = write_tmp(b"not an elf at all, just text");
        assert_eq!(inspect_path(&p), Err(InspectError::Corrupt));
        // Missing file
        assert_eq!(
            inspect_path(Path::new("/nonexistent/x.AppImage")),
            Err(InspectError::NotFound)
        );
    }

    #[test]
    fn no_desktop_file_is_corrupt() {
        let s = Spec {
            entries: vec![Entry::File("/readme.txt", b"hi".to_vec())],
            ..Spec::default()
        };
        let (_d, p) = write_tmp(&build(s));
        assert_eq!(inspect_path(&p), Err(InspectError::Corrupt));
    }

    #[test]
    fn fuse2_heuristic() {
        // Dynamic runtime mentioning libfuse.so.2 => needs it. The marker goes into the
        // zero-filled section table, which is part of the runtime.
        let mut b = build(Spec {
            dynamic: true,
            ..basic()
        });
        let off = elf::ElfHeader::parse(&b)
            .unwrap()
            .squashfs_offset()
            .unwrap() as usize;
        b[off - 20..off - 8].copy_from_slice(b"libfuse.so.2");
        let (_d, p) = write_tmp(&b);
        assert!(inspect_path(&p).unwrap().needs_fuse2);

        // Dynamic without the marker => not flagged.
        let (_d, p) = write_tmp(&build(Spec {
            dynamic: true,
            ..basic()
        }));
        assert!(!inspect_path(&p).unwrap().needs_fuse2);

        // Same marker but static runtime => not needed.
        let mut stat = build(basic());
        let off = elf::ElfHeader::parse(&stat)
            .unwrap()
            .squashfs_offset()
            .unwrap() as usize;
        stat[off - 20..off - 8].copy_from_slice(b"libfuse.so.2");
        let (_d, p) = write_tmp(&stat);
        assert!(!inspect_path(&p).unwrap().needs_fuse2);
    }

    fn fixtures_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }

    /// Regenerates `tests/fixtures/*.AppImage`. Run via `tests/fixtures/make-fixtures.sh`.
    /// There is no `mksquashfs` dependency: backhand's writer builds the image.
    #[test]
    #[ignore = "writes files; run through make-fixtures.sh"]
    fn regenerate_fixtures() {
        let dir = fixtures_dir();
        fs::create_dir_all(&dir).unwrap();
        let put = |name: &str, bytes: Vec<u8>| fs::write(dir.join(name), bytes).unwrap();
        put("lumen-notes-x86_64.AppImage", build(basic()));
        put(
            "lumen-notes-aarch64.AppImage",
            build(Spec {
                machine: 183,
                ..basic()
            }),
        );
        put(
            "type1.AppImage",
            build(Spec {
                ai_type: 1,
                ..basic()
            }),
        );
        let mut dw = build(basic());
        let off = elf::ElfHeader::parse(&dw)
            .unwrap()
            .squashfs_offset()
            .unwrap() as usize;
        dw[off..off + 6].copy_from_slice(b"DWARFS");
        put("dwarfs.AppImage", dw);
        let mut fuse = build(Spec {
            dynamic: true,
            ..basic()
        });
        let off = elf::ElfHeader::parse(&fuse)
            .unwrap()
            .squashfs_offset()
            .unwrap() as usize;
        fuse[off - 20..off - 8].copy_from_slice(b"libfuse.so.2");
        put("fuse2-dynamic.AppImage", fuse);
    }

    #[test]
    fn committed_fixtures_inspect_as_expected() {
        let dir = fixtures_dir();
        if !dir.join("lumen-notes-x86_64.AppImage").exists() {
            eprintln!("fixtures not generated; skipped");
            return;
        }
        let ok = inspect_path(&dir.join("lumen-notes-x86_64.AppImage")).unwrap();
        assert_eq!(ok.display_name, "Lumen Notes");
        assert!(!ok.needs_fuse2);
        let arm = inspect_path(&dir.join("lumen-notes-aarch64.AppImage")).unwrap();
        assert_eq!(arm.arch, installer_domain::Arch::Aarch64);
        assert_eq!(
            inspect_path(&dir.join("type1.AppImage")),
            Err(InspectError::Unsupported(UnsupportedReason::AppImageType1))
        );
        assert_eq!(
            inspect_path(&dir.join("dwarfs.AppImage")),
            Err(InspectError::Unsupported(UnsupportedReason::Dwarfs))
        );
        assert!(
            inspect_path(&dir.join("fuse2-dynamic.AppImage"))
                .unwrap()
                .needs_fuse2
        );
    }

    #[test]
    fn desktop_entry_parser_table() {
        let text = "# c\n[Other]\nName=No\n[Desktop Entry]\nName=Yes\nName[fr]=Oui\nComment=a\\sb\\\\c\nX-AppImage-Version = 2.0\n[Desktop Action x]\nName=Act\n";
        let m = parse_desktop_entry(text);
        assert_eq!(m["Name"], "Yes");
        assert_eq!(m["Comment"], "a b\\c");
        assert_eq!(m["X-AppImage-Version"], "2.0");
        assert!(!m.contains_key("Name[fr]"));
    }

    #[test]
    fn name_and_version_from_file_name() {
        assert_eq!(name_from_file("Lumen-Notes-1.4.2-x86_64"), "Lumen Notes");
        assert_eq!(name_from_file("1.0"), "1.0");
        assert_eq!(
            version_from_file("Lumen-Notes-1.4.2-x86_64").as_deref(),
            Some("1.4.2")
        );
        assert_eq!(version_from_file("Lumen"), None);
    }

    #[tokio::test]
    async fn port_impl_works() {
        let (_d, p) = write_tmp(&build(basic()));
        let insp = AppImageInspector;
        assert!(insp.supports(PackageFormat::AppImage));
        assert!(!insp.supports(PackageFormat::Rpm));
        let file = PackageFile {
            path: p,
            format: PackageFormat::AppImage,
            size_bytes: 1,
        };
        assert_eq!(insp.inspect(&file).await.unwrap().name, "lumen-notes");
    }

    /// ADR-0005 belt and braces: this module and `elf.rs` never spawn processes.
    #[test]
    fn sources_never_spawn_processes() {
        let needles = [
            concat!("std::", "process"),
            concat!("tokio::", "process"),
            concat!("Command", "::new"),
            concat!("use std::", "process"),
        ];
        for f in ["appimage.rs", "elf.rs"] {
            let p = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("src/inspect")
                .join(f);
            let text = fs::read_to_string(&p).unwrap();
            for n in needles {
                assert!(!text.contains(n), "{f} contains {n}");
            }
        }
    }
}
