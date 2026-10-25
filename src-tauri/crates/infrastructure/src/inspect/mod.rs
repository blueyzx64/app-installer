//! Package inspectors. They read bytes and never run anything (ADR-0005).
//!
//! Planned: `rpm.rs`, `deb.rs`, `appimage.rs`, `elf.rs`, and a router that
//! implements `PackageInspector` by dispatching on `PackageFormat`.

pub mod appimage; // lane S3
pub mod elf; // lane S3
// lane S2b
pub mod deb;
pub mod rpm;

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    /// ADR-0005: no process spawning anywhere in this module.
    #[test]
    fn inspect_never_spawns_processes() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/inspect");
        // Split so this file does not match its own scan.
        let needles = [
            concat!("std::", "process"),
            concat!("tokio::", "process"),
            concat!("Command::", "new"),
        ];
        for entry in fs::read_dir(&dir).expect("read inspect dir") {
            let path = entry.expect("dir entry").path();
            if path.extension().is_some_and(|e| e == "rs") {
                let text = fs::read_to_string(&path).expect("read source");
                for needle in needles {
                    assert!(!text.contains(needle), "{} uses {needle}", path.display());
                }
            }
        }
    }
}
