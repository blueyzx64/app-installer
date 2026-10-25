//! R3 review proof (from swarm/R3/review): decompression-bomb guard for `.deb` control archives.
use std::io::Write;

use installer_infrastructure::inspect::deb::DebInspector;

fn tmp(name: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let d = std::env::temp_dir().join(format!("r3-{nanos}-{name}"));
    std::fs::create_dir_all(&d).expect("mkdir");
    d
}

fn tar_entry(name: &str, typeflag: u8, size: usize) -> Vec<u8> {
    let mut h = [0u8; 512];
    h[..name.len()].copy_from_slice(name.as_bytes());
    h[100..107].copy_from_slice(b"0000644");
    h[124..135].copy_from_slice(format!("{size:011o}").as_bytes());
    h[156] = typeflag;
    h[257..262].copy_from_slice(b"ustar");
    h.to_vec()
}

fn ar(members: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut out = b"!<arch>\n".to_vec();
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

const CONTROL: &str = "Package: tool\nVersion: 1.0\nArchitecture: amd64\nDescription: x\n";

fn deb_with_padding(pad_mib: usize, ext: &str) -> Vec<u8> {
    let raw = tar_entry("./big", b'0', pad_mib << 20);
    let mut tail = tar_entry("./control", b'0', CONTROL.len());
    let mut data = CONTROL.as_bytes().to_vec();
    data.resize(512, 0);
    tail.extend(data);
    tail.extend(vec![0u8; 1024]);
    let z = vec![0u8; 1 << 20];
    let packed = if ext == "gz" {
        let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        e.write_all(&raw).expect("w");
        (0..pad_mib).for_each(|_| e.write_all(&z).expect("w"));
        e.write_all(&tail).expect("w");
        e.finish().expect("fin")
    } else {
        let mut e = liblzma::write::XzEncoder::new(Vec::new(), 1);
        e.write_all(&raw).expect("w");
        (0..pad_mib).for_each(|_| e.write_all(&z).expect("w"));
        e.write_all(&tail).expect("w");
        e.finish().expect("fin")
    };
    ar(&[
        ("debian-binary", b"2.0\n".to_vec()),
        (&format!("control.tar.{ext}"), packed),
    ])
}

/// Inflating 300 MiB of control.tar to find `control` must be refused.
#[test]
fn review_r3_deb_control_tar_decompression_unbounded() {
    let d = tmp("debbomb");
    for ext in ["gz", "xz"] {
        let p = d.join(format!("b.{ext}.deb"));
        std::fs::write(&p, deb_with_padding(300, ext)).expect("write");
        let size = std::fs::metadata(&p).expect("meta").len();
        assert!(size < 8 << 20, "{ext}: {size}");
        let r = DebInspector::inspect_path(&p);
        assert_eq!(
            r.map(|m| m.name),
            Err(installer_domain::InspectError::Corrupt),
            "{ext}"
        );
    }
}
