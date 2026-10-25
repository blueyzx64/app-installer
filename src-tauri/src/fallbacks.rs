//! Small shell-side adapters: the "unavailable" system backend (D-020) and the
//! per-format inspector chain. No IO, no `tauri`.

use installer_application::CancelToken;
use installer_application::ports::{InstallObserver, PackageInspector, SystemPackages};
use installer_domain::{
    BackendError, InspectError, InstallError, InstallPlan, PackageFile, PackageFormat,
    PackageMetadata, UnsupportedReason, Version,
};

/// System packages until the PackageKit backend lands (W2, D-020).
/// Nothing is installed, and installing says the backend is unavailable.
#[derive(Debug, Default, Clone, Copy)]
pub struct UnavailableSystemPackages;

impl SystemPackages for UnavailableSystemPackages {
    async fn installed_version(&self, _name: &str) -> Result<Option<Version>, BackendError> {
        Ok(None)
    }

    async fn plan(&self, _file: &PackageFile) -> Result<InstallPlan, BackendError> {
        Err(BackendError::Unavailable("system backend".into()))
    }

    async fn install(
        &self,
        _file: &PackageFile,
        _observer: &dyn InstallObserver,
        _cancel: CancelToken,
    ) -> Result<(), InstallError> {
        Err(InstallError::BackendUnavailable)
    }

    async fn remove(
        &self,
        _name: &str,
        _observer: &dyn InstallObserver,
    ) -> Result<(), InstallError> {
        Err(InstallError::BackendUnavailable)
    }
}

/// Supports no format. End of an [`InspectorChain`].
#[derive(Debug, Default, Clone, Copy)]
pub struct NoInspector;

impl PackageInspector for NoInspector {
    fn supports(&self, _format: PackageFormat) -> bool {
        false
    }

    async fn inspect(&self, _file: &PackageFile) -> Result<PackageMetadata, InspectError> {
        Err(InspectError::Unsupported(UnsupportedReason::UnknownFormat))
    }
}

/// Dispatches by format: `head` first, otherwise `tail`.
/// Build with `InspectorChain::new(appimage, InspectorChain::new(rpm, NoInspector))`.
#[derive(Debug, Clone)]
pub struct InspectorChain<H, T> {
    head: H,
    tail: T,
}

impl<H, T> InspectorChain<H, T> {
    pub fn new(head: H, tail: T) -> Self {
        Self { head, tail }
    }
}

impl<H: PackageInspector, T: PackageInspector> PackageInspector for InspectorChain<H, T> {
    fn supports(&self, format: PackageFormat) -> bool {
        self.head.supports(format) || self.tail.supports(format)
    }

    async fn inspect(&self, file: &PackageFile) -> Result<PackageMetadata, InspectError> {
        if self.head.supports(file.format) {
            self.head.inspect(file).await
        } else {
            self.tail.inspect(file).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use installer_application::testing::{FakeInspector, RecordingObserver};
    use std::path::PathBuf;

    fn file(format: PackageFormat) -> PackageFile {
        PackageFile {
            path: PathBuf::from("/x"),
            format,
            size_bytes: 1,
        }
    }

    #[tokio::test]
    async fn unavailable_system_reports_unavailable() {
        let s = UnavailableSystemPackages;
        assert_eq!(s.installed_version("x").await, Ok(None));
        let r = s
            .install(
                &file(PackageFormat::Rpm),
                &RecordingObserver::default(),
                CancelToken::new(),
            )
            .await;
        assert_eq!(r, Err(InstallError::BackendUnavailable));
    }

    #[tokio::test]
    async fn chain_routes_by_format() {
        let app = FakeInspector::default();
        app.only_formats(&[PackageFormat::AppImage]);
        let chain = InspectorChain::new(app, NoInspector);
        assert!(chain.supports(PackageFormat::AppImage));
        assert!(!chain.supports(PackageFormat::Rpm));
        assert!(matches!(
            chain.inspect(&file(PackageFormat::Rpm)).await,
            Err(InspectError::Unsupported(_))
        ));
    }
}
