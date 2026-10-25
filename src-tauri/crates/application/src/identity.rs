//! App identity (D-037). The ledger is the only source of truth for an `AppId`.
//!
//! An installed app is identified by `(source, key)`. The key is stored in
//! `InstalledApp::name`: the package name for system packages, and for AppImages the
//! embedded `.desktop` stem (or the machine name when there is none). Keys are compared
//! after NFC normalisation (D-038).
//!
//! Pure functions over a ledger snapshot, so install, open and list agree.

use installer_domain::{AppId, AppSource, InstallScope, InstalledApp, PackageMetadata, nfc};

/// Where an app with this metadata lives once installed.
pub fn source_of(meta: &PackageMetadata) -> AppSource {
    match meta.scope {
        InstallScope::System => AppSource::SystemPackage,
        InstallScope::User => AppSource::AppImage,
    }
}

/// The ledger entry for `(source, key)`, if any.
pub fn find<'a>(
    apps: &'a [InstalledApp],
    source: AppSource,
    key: &str,
) -> Option<&'a InstalledApp> {
    let key = nfc(key);
    apps.iter()
        .find(|a| a.source == source && nfc(&a.name) == key)
}

/// The id to use when installing `(source, key)` shown as `display_name`.
///
/// Same identity in the ledger: reuse its id (an update). Otherwise the slug of the
/// display name, with `-2`, `-3`… while any ledger entry (of any source) holds it.
pub fn allocate(apps: &[InstalledApp], source: AppSource, key: &str, display_name: &str) -> AppId {
    if let Some(known) = find(apps, source, key) {
        return known.id.clone();
    }
    let base = AppId::from_name(display_name);
    let free = |id: &AppId| apps.iter().all(|a| &a.id != id);
    if free(&base) {
        return base;
    }
    for n in 2_u32..10_000 {
        if let Some(id) = suffixed(&base, n)
            && free(&id)
        {
            return id;
        }
    }
    base
}

/// `base-n`, trimmed to stay a valid 64-char id.
pub fn suffixed(base: &AppId, n: u32) -> Option<AppId> {
    let suffix = format!("-{n}");
    let room = 64usize.saturating_sub(suffix.len());
    let stem: String = base.as_str().chars().take(room).collect();
    format!("{}{suffix}", stem.trim_end_matches('-'))
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::sample_app;

    fn app(id: &str, key: &str, source: AppSource) -> InstalledApp {
        let mut a = sample_app(id, "1", source);
        a.name = key.to_owned();
        a
    }

    #[test]
    fn same_identity_reuses_id() {
        let apps = vec![app("lumen-2", "org.lumen.Notes", AppSource::AppImage)];
        let id = allocate(&apps, AppSource::AppImage, "org.lumen.Notes", "Lumen Notes");
        assert_eq!(id.as_str(), "lumen-2");
    }

    #[test]
    fn other_identity_with_same_slug_gets_suffix() {
        let apps = vec![
            app("foo-bar", "foo-bar", AppSource::AppImage),
            app("foo-bar-2", "x", AppSource::SystemPackage),
        ];
        // Different key, same slug -> next free suffix.
        let id = allocate(&apps, AppSource::AppImage, "foo.Bar", "Foo-Bar");
        assert_eq!(id.as_str(), "foo-bar-3");
        // Same key but other source is another app.
        let id = allocate(&apps, AppSource::SystemPackage, "foo-bar", "Foo Bar");
        assert_eq!(id.as_str(), "foo-bar-3");
    }

    #[test]
    fn free_slug_is_used_as_is_and_keys_compare_nfc() {
        assert_eq!(
            allocate(&[], AppSource::AppImage, "k", "Lumen").as_str(),
            "lumen"
        );
        let apps = vec![app("cafe", "caf\u{e9}", AppSource::AppImage)];
        assert!(find(&apps, AppSource::AppImage, "cafe\u{301}").is_some());
        assert!(find(&apps, AppSource::SystemPackage, "caf\u{e9}").is_none());
    }

    #[test]
    fn suffix_keeps_id_valid() {
        let long: AppId = "a".repeat(64).parse().unwrap();
        let s = suffixed(&long, 12).unwrap();
        assert_eq!(s.as_str().len(), 64);
        assert!(s.as_str().ends_with("-12"));
    }
}
