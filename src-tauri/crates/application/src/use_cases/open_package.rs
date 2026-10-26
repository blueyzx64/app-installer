use std::path::PathBuf;
use std::sync::Arc;

use installer_domain::{
    AppId, Arch, Compatibility, InspectError, InstallScope, InstalledApp, PackageFile,
    PackageFormat, PackageMetadata, PackagePreview, SNIFF_LEN, UnsupportedReason, Version,
};

use crate::PreviewRegistry;
use crate::identity;
use crate::ports::{AppLedger, FileProbe, HostProbe, PackageInspector, SystemPackages};

/// Read a package file and build the preview shown before installing.
///
/// Parses headers only. Nothing is executed and nothing is installed.
pub struct OpenPackage<P, I, H, S, L> {
    probe: Arc<P>,
    inspector: Arc<I>,
    host: Arc<H>,
    system: Arc<S>,
    ledger: Arc<L>,
    registry: Arc<PreviewRegistry>,
}

impl<P, I, H, S, L> OpenPackage<P, I, H, S, L>
where
    P: FileProbe,
    I: PackageInspector,
    H: HostProbe,
    S: SystemPackages,
    L: AppLedger,
{
    pub fn new(
        probe: Arc<P>,
        inspector: Arc<I>,
        host: Arc<H>,
        system: Arc<S>,
        ledger: Arc<L>,
        registry: Arc<PreviewRegistry>,
    ) -> Self {
        Self {
            probe,
            inspector,
            host,
            system,
            ledger,
            registry,
        }
    }

    /// Errors: unreadable or damaged files, and files that are not a package at all
    /// (`Unsupported(UnknownFormat)`). Known-but-unsupported packages (AppImage
    /// type 1, DwarFS) succeed with `Compatibility::Unsupported` so the UI can explain.
    #[tracing::instrument(skip(self, path))]
    pub async fn execute(&self, path: PathBuf) -> Result<PackagePreview, InspectError> {
        let head = self.probe.head(&path, SNIFF_LEN).await?;
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let format = PackageFormat::sniff(&head.bytes, &file_name)
            .filter(|f| self.inspector.supports(*f))
            .ok_or(InspectError::Unsupported(UnsupportedReason::UnknownFormat))?;
        let file = PackageFile {
            path,
            format,
            size_bytes: head.size_bytes,
        };
        let host = self.host.host();

        let (meta, compat, installed_app_id) = match self.inspector.inspect(&file).await {
            Ok(meta) => {
                let ledger = self.ledger_snapshot().await;
                let known = identity::find(&ledger, identity::source_of(&meta), &meta.name);
                let installed = self.installed_version(&meta, known).await;
                let compat = Compatibility::evaluate(&meta, &host, installed.as_ref());
                let installed_app_id = match compat {
                    Compatibility::AlreadyInstalled { .. } => known.map(|a| a.id.clone()),
                    _ => None,
                };
                (meta, compat, installed_app_id)
            }
            Err(InspectError::Unsupported(reason)) => (
                placeholder(&file, &file_name),
                Compatibility::Unsupported { reason },
                None,
            ),
            Err(e) => return Err(e),
        };

        let needs_permission = meta.scope.needs_permission();
        Ok(self.registry.insert(file.clone(), |id| PackagePreview {
            id,
            size_bytes: file.size_bytes,
            meta,
            compat,
            needs_permission,
            installed_app_id,
        }))
    }

    /// The ledger, or empty when it cannot be read (open must not fail on it).
    async fn ledger_snapshot(&self) -> Vec<InstalledApp> {
        self.ledger.list().await.unwrap_or_else(|e| {
            tracing::warn!(error = %e, "ledger unreadable; assuming not installed");
            Vec::new()
        })
    }

    /// Where "already installed" is looked up depends on scope: the package manager for
    /// system packages, the ledger identity (D-037) for AppImages. A lookup failure is
    /// not fatal: we assume "not installed" and the backend will sort it out.
    async fn installed_version(
        &self,
        meta: &PackageMetadata,
        known: Option<&InstalledApp>,
    ) -> Option<Version> {
        match meta.scope {
            InstallScope::System => match self.system.installed_version(&meta.name).await {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(error = %e, "installed_version failed; assuming not installed");
                    None
                }
            },
            InstallScope::User => known.map(|a| a.version.clone()),
        }
    }
}

/// Stand-in metadata for packages we cannot read, so the UI still has a name to show.
fn placeholder(file: &PackageFile, file_name: &str) -> PackageMetadata {
    let stem = file_name
        .rsplit_once('.')
        .map_or(file_name, |(stem, _)| stem);
    PackageMetadata {
        format: file.format,
        name: AppId::from_name(stem).as_str().to_owned(),
        display_name: stem.to_owned(),
        version: Version::new(""),
        arch: Arch::Noarch,
        summary: None,
        icon: None,
        scope: InstallScope::for_format(file.format),
        needs_fuse2: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::*;
    use installer_domain::{AppSource, BackendError, DistroFamily, InstallScope, VersionRelation};

    struct Rig {
        probe: Arc<FakeFileProbe>,
        inspector: Arc<FakeInspector>,
        host: Arc<FakeHost>,
        system: Arc<FakeSystemPackages>,
        ledger: Arc<FakeLedger>,
        registry: Arc<PreviewRegistry>,
    }

    impl Rig {
        fn new() -> Self {
            Self {
                probe: Arc::new(FakeFileProbe::default()),
                inspector: Arc::new(FakeInspector::default()),
                host: Arc::new(FakeHost::fedora()),
                system: Arc::new(FakeSystemPackages::default()),
                ledger: Arc::new(FakeLedger::default()),
                registry: Arc::new(PreviewRegistry::new()),
            }
        }

        fn uc(
            &self,
        ) -> OpenPackage<FakeFileProbe, FakeInspector, FakeHost, FakeSystemPackages, FakeLedger>
        {
            OpenPackage::new(
                self.probe.clone(),
                self.inspector.clone(),
                self.host.clone(),
                self.system.clone(),
                self.ledger.clone(),
                self.registry.clone(),
            )
        }
    }

    fn rpm_path() -> PathBuf {
        PathBuf::from("/home/u/Downloads/lumen-notes.rpm")
    }

    #[tokio::test]
    async fn happy_path_rpm_is_stored_and_needs_permission() {
        let rig = Rig::new();
        rig.probe.add(&rpm_path(), RPM_HEAD, 1234);
        rig.inspector.set_meta(sample_meta(PackageFormat::Rpm));

        let preview = rig.uc().execute(rpm_path()).await.unwrap();

        assert_eq!(preview.compat, Compatibility::Ok);
        assert!(preview.needs_permission);
        assert_eq!(preview.size_bytes, 1234);
        assert_eq!(preview.meta.display_name, "Lumen Notes");
        let stored = rig.registry.get(preview.id).unwrap();
        assert_eq!(stored.file.format, PackageFormat::Rpm);
        assert_eq!(stored.file.path, rpm_path());
    }

    #[tokio::test]
    async fn appimage_is_user_scope_no_permission() {
        let rig = Rig::new();
        let path = PathBuf::from("/tmp/Lumen.AppImage");
        rig.probe.add(&path, APPIMAGE_HEAD, 10);
        rig.inspector.set_meta(sample_meta(PackageFormat::AppImage));

        let preview = rig.uc().execute(path).await.unwrap();
        assert!(!preview.needs_permission);
        assert_eq!(preview.meta.scope, InstallScope::User);
    }

    #[tokio::test]
    async fn wrong_family_is_reported_not_failed() {
        let rig = Rig::new();
        let path = PathBuf::from("/tmp/a.deb");
        rig.probe.add(&path, DEB_HEAD, 10);
        rig.inspector.set_meta(sample_meta(PackageFormat::Deb));

        let preview = rig.uc().execute(path).await.unwrap();
        assert_eq!(
            preview.compat,
            Compatibility::WrongFamily {
                made_for: DistroFamily::DebBased
            }
        );
    }

    #[tokio::test]
    async fn installed_system_version_marks_update() {
        let rig = Rig::new();
        rig.probe.add(&rpm_path(), RPM_HEAD, 1);
        rig.inspector.set_meta(sample_meta(PackageFormat::Rpm)); // version 2.0
        rig.system.set_installed("lumen-notes", "1.0");

        let preview = rig.uc().execute(rpm_path()).await.unwrap();
        assert_eq!(
            preview.compat,
            Compatibility::AlreadyInstalled {
                relation: VersionRelation::Newer,
                installed: Version::new("1.0")
            }
        );
    }

    #[tokio::test]
    async fn appimage_installed_state_comes_from_ledger() {
        let rig = Rig::new();
        let path = PathBuf::from("/tmp/Lumen.AppImage");
        rig.probe.add(&path, APPIMAGE_HEAD, 10);
        rig.inspector.set_meta(sample_meta(PackageFormat::AppImage));
        rig.ledger.seed(sample_app(
            "lumen-notes",
            "2.0",
            installer_domain::AppSource::AppImage,
        ));

        let preview = rig.uc().execute(path).await.unwrap();
        assert!(matches!(
            preview.compat,
            Compatibility::AlreadyInstalled {
                relation: VersionRelation::Same,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn installed_app_id_comes_from_ledger_identity() {
        let rig = Rig::new();
        let path = PathBuf::from("/tmp/Lumen.AppImage");
        rig.probe.add(&path, APPIMAGE_HEAD, 10);
        rig.inspector.set_meta(sample_meta(PackageFormat::AppImage)); // key lumen-notes
        // Same key, suffixed id: the id is the ledger's, not the recomputed slug.
        let mut app = sample_app("lumen-notes-2", "1.0", AppSource::AppImage);
        app.name = "lumen-notes".into();
        rig.ledger.seed(app);
        // Same key under another source is a different app.
        rig.ledger
            .seed(sample_app("other", "1.0", AppSource::SystemPackage));

        let preview = rig.uc().execute(path).await.unwrap();
        assert_eq!(
            preview.installed_app_id.as_ref().map(AppId::as_str),
            Some("lumen-notes-2")
        );
    }

    #[tokio::test]
    async fn installed_app_id_is_none_when_not_installed_or_not_in_ledger() {
        let rig = Rig::new();
        rig.probe.add(&rpm_path(), RPM_HEAD, 1);
        rig.inspector.set_meta(sample_meta(PackageFormat::Rpm));
        // Installed on the system, but not by us.
        rig.system.set_installed("lumen-notes", "1.0");
        let preview = rig.uc().execute(rpm_path()).await.unwrap();
        assert!(matches!(
            preview.compat,
            Compatibility::AlreadyInstalled { .. }
        ));
        assert_eq!(preview.installed_app_id, None);
        // Now in the ledger too.
        rig.ledger
            .seed(sample_app("lumen-notes", "1.0", AppSource::SystemPackage));
        let preview = rig.uc().execute(rpm_path()).await.unwrap();
        assert_eq!(
            preview.installed_app_id.as_ref().map(AppId::as_str),
            Some("lumen-notes")
        );
        // Fresh AppImage: not installed.
        let rig = Rig::new();
        let path = PathBuf::from("/tmp/Lumen.AppImage");
        rig.probe.add(&path, APPIMAGE_HEAD, 10);
        rig.inspector.set_meta(sample_meta(PackageFormat::AppImage));
        assert_eq!(rig.uc().execute(path).await.unwrap().installed_app_id, None);
    }

    #[tokio::test]
    async fn lookup_failures_are_not_fatal() {
        let rig = Rig::new();
        rig.probe.add(&rpm_path(), RPM_HEAD, 1);
        rig.inspector.set_meta(sample_meta(PackageFormat::Rpm));
        rig.system
            .fail_installed_version(BackendError::Unavailable("pk".into()));

        let preview = rig.uc().execute(rpm_path()).await.unwrap();
        assert_eq!(preview.compat, Compatibility::Ok);
    }

    #[tokio::test]
    async fn unsupported_appimage_gets_placeholder_preview() {
        let rig = Rig::new();
        let path = PathBuf::from("/tmp/Old Thing.AppImage");
        rig.probe.add(&path, APPIMAGE_HEAD, 10);
        rig.inspector
            .fail(InspectError::Unsupported(UnsupportedReason::AppImageType1));

        let preview = rig.uc().execute(path).await.unwrap();
        assert_eq!(
            preview.compat,
            Compatibility::Unsupported {
                reason: UnsupportedReason::AppImageType1
            }
        );
        assert_eq!(preview.meta.display_name, "Old Thing");
        assert_eq!(preview.meta.name, "old-thing");
    }

    #[tokio::test]
    async fn unknown_bytes_are_unsupported_whatever_the_extension() {
        let rig = Rig::new();
        rig.probe.add(&rpm_path(), b"just text", 9);
        rig.inspector.set_meta(sample_meta(PackageFormat::Rpm));

        let err = rig.uc().execute(rpm_path()).await.unwrap_err();
        assert_eq!(
            err,
            InspectError::Unsupported(UnsupportedReason::UnknownFormat)
        );
        assert!(rig.registry.is_empty());
    }

    #[tokio::test]
    async fn missing_file_and_corrupt_package_fail() {
        let rig = Rig::new();
        let err = rig.uc().execute(rpm_path()).await.unwrap_err();
        assert_eq!(err, InspectError::NotFound);

        rig.probe.add(&rpm_path(), RPM_HEAD, 1);
        rig.inspector.fail(InspectError::Corrupt);
        let err = rig.uc().execute(rpm_path()).await.unwrap_err();
        assert_eq!(err, InspectError::Corrupt);
        assert!(rig.registry.is_empty());
    }

    #[tokio::test]
    async fn format_without_inspector_is_unsupported() {
        let rig = Rig::new();
        rig.inspector.only_formats(&[PackageFormat::AppImage]);
        rig.probe.add(&rpm_path(), RPM_HEAD, 1);
        let err = rig.uc().execute(rpm_path()).await.unwrap_err();
        assert_eq!(
            err,
            InspectError::Unsupported(UnsupportedReason::UnknownFormat)
        );
    }
}
