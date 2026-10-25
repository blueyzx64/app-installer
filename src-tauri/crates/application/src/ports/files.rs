use std::path::Path;

use installer_domain::{InspectError, PackageFile, PackageFormat, PackageMetadata};

/// The first bytes of a file plus its total size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileHead {
    pub bytes: Vec<u8>,
    pub size_bytes: u64,
}

/// Reads file heads so the domain can sniff the format. Keeps `std::fs` out of use cases.
#[trait_variant::make(Send)]
pub trait FileProbe: Sync {
    /// Read up to `len` bytes from the start of `path`.
    async fn head(&self, path: &Path, len: usize) -> Result<FileHead, InspectError>;
}

/// Reads package metadata without ever executing the package (ADR-0005).
#[trait_variant::make(Send)]
pub trait PackageInspector: Sync {
    fn supports(&self, format: PackageFormat) -> bool;
    async fn inspect(&self, file: &PackageFile) -> Result<PackageMetadata, InspectError>;
}
