//! R1 adversarial proofs for AppImage inspection (F-03, F-04, F-15, ADR-0005).
//! Tests named `review_r1_*`. A failing test = a finding.
use std::io::Cursor;
use std::path::Path;
use std::time::Instant;

use backhand::{FilesystemWriter, NodeHeader};
use installer_domain::PackageFormat;
use installer_infrastructure::inspect::appimage::inspect_path;

fn elf_stub(machine: u16, dynamic: bool) -> Vec<u8> {
    let (hdr, phent, shent) = (64usize, 56usize, 64usize);
    let phnum = usize::from(dynamic);
    let shoff = hdr + phnum * phent;
    let mut b = vec![0u8; shoff + 2 * shent];
    b[..4].copy_from_slice(b"\x7fELF");
    b[4] = 2;
    b[5] = 1;
    b[6] = 1;
    b[8..11].copy_from_slice(b"AI\x02");
    b[18..20].copy_from_slice(&machine.to_le_bytes());
    b[32..40].copy_from_slice(&(hdr as u64).to_le_bytes());
    b[40..48].copy_from_slice(&(shoff as u64).to_le_bytes());
    b[52..54].copy_from_slice(&64u16.to_le_bytes());
    b[54..56].copy_from_slice(&(phent as u16).to_le_bytes());
    b[56..58].copy_from_slice(&(phnum as u16).to_le_bytes());
    b[58..60].copy_from_slice(&64u16.to_le_bytes());
    b[60..62].copy_from_slice(&2u16.to_le_bytes());
    if dynamic {
        b[hdr..hdr + 4].copy_from_slice(&3u32.to_le_bytes());
    }
    b
}

fn squash(entries: Vec<(&str, Vec<u8>)>) -> Vec<u8> {
    let mut w = FilesystemWriter::default();
    let h = NodeHeader::new(0o644, 0, 0, 0);
    for (p, d) in entries {
        w.push_file(Cursor::new(d), p, h).unwrap();
    }
    let mut c = Cursor::new(Vec::new());
    w.write(&mut c).unwrap();
    c.into_inner()
}

fn image(entries: Vec<(&str, Vec<u8>)>) -> Vec<u8> {
    let mut o = elf_stub(62, false);
    o.extend(squash(entries));
    o
}

fn desk(name: &str) -> Vec<u8> {
    format!("[Desktop Entry]\nType=Application\nName={name}\nIcon=a\nExec=AppRun\n").into_bytes()
}

fn tmp(bytes: &[u8], name: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join(name);
    std::fs::write(&p, bytes).unwrap();
    (d, p)
}

/// Display name must be one clean line, no control / bidi override chars.
#[test]
fn review_r1_name_escape_newline_and_controls_sanitised() {
    let (_d, p) = tmp(
        &image(vec![("/a.desktop", desk("Evil\\nApp\\t\u{202e}gpj"))]),
        "x.AppImage",
    );
    let m = inspect_path(&p).unwrap();
    assert!(
        !m.display_name
            .chars()
            .any(|c| c.is_control() || c == '\u{202e}'),
        "display_name has control/bidi chars: {:?}",
        m.display_name
    );
}

#[test]
fn review_r1_name_length_bounded() {
    let long = "A".repeat(1_500_000);
    let (_d, p) = tmp(&image(vec![("/a.desktop", desk(&long))]), "x.AppImage");
    let m = inspect_path(&p).unwrap();
    assert!(
        m.display_name.chars().count() <= 256,
        "len {}",
        m.display_name.len()
    );
}

#[test]
fn review_r1_comment_length_bounded() {
    let long = "C".repeat(1_500_000);
    let d = format!("[Desktop Entry]\nName=A\nComment={long}\n").into_bytes();
    let (_dd, p) = tmp(&image(vec![("/a.desktop", d)]), "x.AppImage");
    let m = inspect_path(&p).unwrap();
    assert!(m.summary.unwrap().chars().count() <= 1024);
}

fn png_dims(b: &[u8]) -> (u32, u32) {
    (
        u32::from_be_bytes(b[16..20].try_into().unwrap()),
        u32::from_be_bytes(b[20..24].try_into().unwrap()),
    )
}

fn crc(d: &[u8]) -> u32 {
    let mut c = 0xffff_ffffu32;
    for &b in d {
        c ^= u32::from(b);
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xedb8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
    }
    !c
}

/// Icon PNG whose header claims huge dimensions (decompression bomb) must not pass through unscaled.
#[test]
fn review_r1_png_bomb_not_passed_through() {
    use std::io::Write;
    let (w, h) = (30000u32, 30000u32);
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let chunk = |png: &mut Vec<u8>, ty: &[u8], data: &[u8]| {
        png.extend((data.len() as u32).to_be_bytes());
        let mut c = ty.to_vec();
        c.extend(data);
        png.extend(&c);
        png.extend(crc(&c).to_be_bytes());
    };
    let mut ihdr = Vec::new();
    ihdr.extend(w.to_be_bytes());
    ihdr.extend(h.to_be_bytes());
    ihdr.extend([8, 0, 0, 0, 0]);
    chunk(&mut png, b"IHDR", &ihdr);
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
    let row = vec![0u8; 30001];
    for _ in 0..h {
        z.write_all(&row).unwrap();
    }
    chunk(&mut png, b"IDAT", &z.finish().unwrap());
    chunk(&mut png, b"IEND", &[]);
    assert!(png.len() < 2 * 1024 * 1024, "bomb {} too big", png.len());
    let (_d, p) = tmp(
        &image(vec![("/a.desktop", desk("A")), ("/.DirIcon", png)]),
        "x.AppImage",
    );
    let t = Instant::now();
    let m = inspect_path(&p).unwrap();
    let el = t.elapsed();
    if let Some(i) = m.icon {
        let (iw, ih) = png_dims(&i.bytes);
        assert!(
            iw <= 256 && ih <= 256,
            "icon passed through unscaled {iw}x{ih} ({el:?})"
        );
    }
}

/// Hostile ELF headers must give an error, never panic or hang.
#[test]
fn review_r1_hostile_elf_headers_never_panic() {
    let base = image(vec![("/a.desktop", desk("A"))]);
    let mut cases: Vec<(String, Vec<u8>)> = vec![];
    for (name, off, bytes) in [
        ("shoff=max", 40usize, u64::MAX.to_le_bytes().to_vec()),
        ("shoff=2^63", 40, (1u64 << 63).to_le_bytes().to_vec()),
        ("shentsize/shnum=ffff", 58, vec![0xff; 4]),
        (
            "shoff=filelen",
            40,
            (base.len() as u64).to_le_bytes().to_vec(),
        ),
    ] {
        let mut b = base.clone();
        b[off..off + bytes.len()].copy_from_slice(&bytes);
        cases.push((name.into(), b));
    }
    let mut be = base.clone();
    be[5] = 2;
    cases.push(("big-endian".into(), be));
    let mut e32 = base.clone();
    e32[4] = 1;
    cases.push(("elf32-with-64-body".into(), e32));
    cases.push(("only-52-bytes".into(), base[..52].to_vec()));
    cases.push(("only-10-bytes".into(), base[..10].to_vec()));
    cases.push(("empty".into(), vec![]));
    cases.push(("63-bytes".into(), base[..63].to_vec()));
    for (n, b) in cases {
        let (_d, p) = tmp(&b, "h.AppImage");
        let t = Instant::now();
        let r = std::panic::catch_unwind(|| inspect_path(&p));
        assert!(r.is_ok(), "PANIC on {n}");
        assert!(t.elapsed().as_secs() < 5, "slow on {n}");
        assert!(r.unwrap().is_err(), "{n} accepted");
    }
}

/// Flip bytes inside the SquashFS: never panic or hang.
#[test]
fn review_r1_squashfs_bitflip_fuzz_no_panic() {
    let base = image(vec![
        ("/a.desktop", desk("A")),
        ("/.DirIcon", b"<svg/>".to_vec()),
    ]);
    let start = elf_stub(62, false).len();
    let mut seed = 0x1234_5678_9abc_def1u64;
    let mut rnd = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let t = Instant::now();
    for i in 0..400 {
        let mut b = base.clone();
        for _ in 0..1 + (rnd() % 4) {
            let at = start + (rnd() as usize % (b.len() - start));
            b[at] = rnd() as u8;
        }
        let (_d, p) = tmp(&b, "f.AppImage");
        let r = std::panic::catch_unwind(|| inspect_path(&p));
        assert!(r.is_ok(), "PANIC iteration {i}");
    }
    assert!(t.elapsed().as_secs() < 60);
}

/// Squashfs superblock claims enormous counts / sizes.
#[test]
fn review_r1_squashfs_superblock_huge_fields() {
    let base = image(vec![("/a.desktop", desk("A"))]);
    let start = elf_stub(62, false).len();
    for (off, len) in [
        (4usize, 4usize),
        (12, 4),
        (26, 2),
        (40, 8),
        (48, 8),
        (56, 8),
        (64, 8),
        (72, 8),
        (80, 8),
    ] {
        let mut b = base.clone();
        for k in 0..len {
            b[start + off + k] = 0xff;
        }
        let (_d, p) = tmp(&b, "s.AppImage");
        let t = Instant::now();
        let r = std::panic::catch_unwind(|| inspect_path(&p));
        assert!(r.is_ok(), "PANIC field@{off}");
        assert!(
            t.elapsed().as_secs() < 5,
            "slow field@{off}: {:?}",
            t.elapsed()
        );
    }
}

#[test]
fn review_r1_oversize_desktop_skipped_not_fatal() {
    let big = vec![b'x'; 3 * 1024 * 1024];
    let (_d, p) = tmp(
        &image(vec![("/a.desktop", big), ("/b.desktop", desk("Second"))]),
        "x.AppImage",
    );
    assert_eq!(inspect_path(&p).unwrap().display_name, "Second");
}

#[test]
fn review_r1_nonutf8_desktop_ok() {
    let mut d = b"[Desktop Entry]\nName=Caf".to_vec();
    d.extend([0xe9, 0xff, 0xfe]);
    d.extend(b"\nExec=x\n");
    let (_d, p) = tmp(&image(vec![("/a.desktop", d)]), "x.AppImage");
    assert!(inspect_path(&p).is_ok());
}

/// F-03: no extension still inspects; sniff says AppImage.
#[test]
fn review_r1_renamed_without_extension() {
    let b = image(vec![("/a.desktop", desk("Renamed App"))]);
    let (_d, p) = tmp(&b, "download");
    assert_eq!(
        PackageFormat::sniff(&b[..64], "download"),
        Some(PackageFormat::AppImage)
    );
    assert_eq!(inspect_path(&p).unwrap().display_name, "Renamed App");
}

/// F-15: dynamic runtime preferring fuse3 that only mentions fuse2 as fallback should not be flagged.
#[test]
fn review_r1_fuse2_false_positive_fuse3_runtime() {
    let mut o = elf_stub(62, true);
    let n = o.len();
    o[n - 60..n - 47].copy_from_slice(b"libfuse3.so.3");
    o[n - 30..n - 18].copy_from_slice(b"libfuse.so.2");
    o.extend(squash(vec![("/a.desktop", desk("A"))]));
    let (_d, p) = tmp(&o, "x.AppImage");
    assert!(
        !inspect_path(&p).unwrap().needs_fuse2,
        "fuse3-capable runtime flagged as needing fuse2"
    );
}

/// Perf: 200 MB image inspect < 150 ms.
#[test]
fn review_r1_perf_200mb() {
    use std::io::Write;
    struct Rng(u64, u64);
    impl std::io::Read for Rng {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let n = buf.len().min(self.1 as usize);
            for b in &mut buf[..n] {
                self.0 ^= self.0 << 13;
                self.0 ^= self.0 >> 7;
                self.0 ^= self.0 << 17;
                *b = self.0 as u8;
            }
            self.1 -= n as u64;
            Ok(n)
        }
    }
    let mut w = FilesystemWriter::default();
    let h = NodeHeader::new(0o644, 0, 0, 0);
    w.push_file(Cursor::new(desk("Big")), "/a.desktop", h)
        .unwrap();
    w.push_file(
        Rng(0x9e37_79b9_7f4a_7c15, 200 * 1024 * 1024),
        "/payload.bin",
        h,
    )
    .unwrap();
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("big.AppImage");
    let mut c = Cursor::new(Vec::new());
    w.write(&mut c).unwrap();
    let mut f = std::fs::File::create(&p).unwrap();
    f.write_all(&elf_stub(62, false)).unwrap();
    f.write_all(&c.into_inner()).unwrap();
    drop(f);
    assert!(std::fs::metadata(&p).unwrap().len() > 150 * 1024 * 1024);
    let mut best = u128::MAX;
    for _ in 0..3 {
        let t = Instant::now();
        inspect_path(&p).unwrap();
        best = best.min(t.elapsed().as_millis());
    }
    eprintln!("200MB inspect best-of-3: {best} ms");
    assert!(best < 150, "inspect took {best} ms");
}

/// ADR-0005: no process spawn anywhere in src/inspect.
#[test]
fn review_r1_no_process_spawn_in_inspect() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/inspect");
    for e in std::fs::read_dir(dir).unwrap() {
        let t = std::fs::read_to_string(e.unwrap().path()).unwrap();
        for line in t.lines().filter(|l| !l.contains("concat!")) {
            assert!(
                !line.contains("std::process") && !line.contains("Command::new"),
                "{line}"
            );
        }
    }
}
