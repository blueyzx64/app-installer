//! Real `FileProbe`: canonicalize, check regular file, read the first bytes.

use std::fs::{self, File};
use std::io::{ErrorKind, Read};
use std::path::Path;

use installer_application::ports::{FileHead, FileProbe};
use installer_domain::InspectError;

#[derive(Debug, Clone, Copy, Default)]
pub struct FsFileProbe;

fn map_io(e: std::io::Error) -> InspectError {
    match e.kind() {
        ErrorKind::NotFound | ErrorKind::PermissionDenied | ErrorKind::NotADirectory => {
            InspectError::NotFound
        }
        _ => InspectError::Io(e.to_string()),
    }
}

impl FsFileProbe {
    fn read_head(path: &Path, len: usize) -> Result<FileHead, InspectError> {
        let real = fs::canonicalize(path).map_err(map_io)?;
        let meta = fs::metadata(&real).map_err(map_io)?;
        if !meta.is_file() {
            return Err(InspectError::NotFound);
        }
        let mut bytes = Vec::new();
        File::open(&real)
            .map_err(map_io)?
            .take(len as u64)
            .read_to_end(&mut bytes)
            .map_err(map_io)?;
        Ok(FileHead {
            bytes,
            size_bytes: meta.len(),
        })
    }
}

impl FileProbe for FsFileProbe {
    async fn head(&self, path: &Path, len: usize) -> Result<FileHead, InspectError> {
        // A tiny read; fine to do inline.
        Self::read_head(path, len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("installer-probe-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[tokio::test]
    async fn reads_head_and_size() {
        let d = tmp("head");
        let f = d.join("a.bin");
        fs::write(&f, b"0123456789").unwrap();
        let h = FsFileProbe.head(&f, 4).await.unwrap();
        assert_eq!(h.bytes, b"0123");
        assert_eq!(h.size_bytes, 10);
        let all = FsFileProbe.head(&f, 100).await.unwrap();
        assert_eq!(all.bytes.len(), 10);
        let _ = fs::remove_dir_all(d);
    }

    #[tokio::test]
    async fn follows_symlinks() {
        let d = tmp("link");
        let f = d.join("real");
        fs::write(&f, b"xyz").unwrap();
        let l = d.join("link");
        std::os::unix::fs::symlink(&f, &l).unwrap();
        assert_eq!(FsFileProbe.head(&l, 8).await.unwrap().bytes, b"xyz");
        let _ = fs::remove_dir_all(d);
    }

    #[tokio::test]
    async fn missing_and_directory_are_not_found() {
        let d = tmp("nf");
        assert_eq!(
            FsFileProbe.head(&d.join("nope"), 4).await.unwrap_err(),
            InspectError::NotFound
        );
        assert_eq!(
            FsFileProbe.head(&d, 4).await.unwrap_err(),
            InspectError::NotFound
        );
        let _ = fs::remove_dir_all(d);
    }
}
