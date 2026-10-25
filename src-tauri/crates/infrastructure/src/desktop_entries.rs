//! `DesktopIntegration` (F-22): generated `.desktop` entries and hicolor icons.
//!
//! We write `<data_home>/applications/<APP_PREFIX>.<slug>.desktop` and icons under
//! `<data_home>/icons/hicolor/<size>/apps/`. The entry is generated from scratch; the
//! `Exec=` inside the image is never copied (ADR-0005).
//!
//! We do not run `update-desktop-database` (no shelling out). GNOME Shell watches the
//! applications directory with inotify, so a new entry shows up in the app grid on its own.
//! HUMAN: confirm on a real GNOME session that the app appears in the grid without relogin.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use installer_application::ports::DesktopIntegration;
use installer_domain::{IconData, IconKind, InstallError, InstalledApp};

/// Desktop id / icon name prefix. Matches the app identifier.
pub const APP_PREFIX: &str = "io.github.blueyzx64.Installer";

/// Icon sizes we render for PNG sources.
pub const ICON_SIZES: [u32; 3] = [48, 128, 256];

/// `io.github.blueyzx64.Installer.<slug>`: desktop file stem and icon name.
pub fn desktop_stem(slug: &str) -> String {
    format!("{APP_PREFIX}.{slug}")
}

/// Escape a Desktop Entry `string` value: `\`, newline, tab and CR; other control
/// characters are dropped.
pub fn escape_value(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

/// Quote one `Exec` argument: wrap in double quotes and backslash-escape `"`, `` ` ``,
/// `$` and `\`. A literal `%` becomes `%%` so it is not read as a field code.
/// The result must still go through [`escape_value`] before it is written.
pub fn quote_exec_arg(arg: &str) -> String {
    let mut out = String::with_capacity(arg.len() + 2);
    out.push('"');
    for c in arg.chars() {
        match c {
            '"' | '`' | '$' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            '%' => out.push_str("%%"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Which PNG sizes to write for a square-ish source of `side` pixels.
/// Every standard size that fits, plus the next one up when it needs at most about a third
/// of upscaling; a tiny source still gets the smallest size.
pub fn icon_sizes(side: u32) -> Vec<u32> {
    let mut sizes: Vec<u32> = ICON_SIZES.iter().copied().filter(|s| *s <= side).collect();
    if let Some(next) = ICON_SIZES.iter().copied().find(|s| *s > side)
        && (sizes.is_empty() || side * 4 >= next * 3)
    {
        sizes.push(next);
    }
    sizes
}

/// Render the `.desktop` file text for an app.
pub fn render_entry(name: &str, exec_path: &Path, icon_name: &str) -> String {
    let path = exec_path.to_string_lossy();
    let exec = format!("{} %U", quote_exec_arg(&path));
    let mut s = String::from("[Desktop Entry]\nType=Application\nVersion=1.0\n");
    let mut line = |k: &str, v: &str| {
        s.push_str(k);
        s.push('=');
        s.push_str(v);
        s.push('\n');
    };
    line("Name", &escape_value(name));
    line("Exec", &escape_value(&exec));
    line("TryExec", &escape_value(&path));
    line("Icon", &escape_value(icon_name));
    line("Terminal", "false");
    line("X-Installer-Managed", "true");
    line("X-Installer-Source", &escape_value(&path));
    s
}

#[derive(Debug, Clone)]
pub struct DesktopEntries {
    data_home: PathBuf,
}

impl DesktopEntries {
    /// `data_home` is `$XDG_DATA_HOME` (injected so tests use a temp dir).
    pub fn new(data_home: impl Into<PathBuf>) -> Self {
        Self {
            data_home: data_home.into(),
        }
    }

    /// `$XDG_DATA_HOME`, else `~/.local/share`.
    pub fn from_env() -> Option<Self> {
        Some(Self::new(default_data_home()?))
    }

    fn applications_dir(&self) -> PathBuf {
        self.data_home.join("applications")
    }

    fn icon_path(&self, size_dir: &str, stem: &str, ext: &str) -> PathBuf {
        self.data_home
            .join("icons/hicolor")
            .join(size_dir)
            .join("apps")
            .join(format!("{stem}.{ext}"))
    }

    fn write_blocking(
        &self,
        app: &InstalledApp,
        icon: Option<&IconData>,
    ) -> Result<String, InstallError> {
        let path = app
            .path
            .as_deref()
            .filter(|p| p.is_absolute())
            .ok_or_else(|| InstallError::Unknown("app has no absolute path".into()))?;
        let stem = desktop_stem(app.id.as_str());
        // Replace any icons from an earlier version.
        self.remove_icons(&stem).map_err(io_to_install)?;
        if let Some(icon) = icon {
            self.write_icons(&stem, icon)?;
        }
        let text = render_entry(&app.display_name, path, &stem);
        let file = self.applications_dir().join(format!("{stem}.desktop"));
        write_atomic(&file, text.as_bytes(), 0o644).map_err(io_to_install)?;
        Ok(format!("{stem}.desktop"))
    }

    fn write_icons(&self, stem: &str, icon: &IconData) -> Result<(), InstallError> {
        match icon.kind {
            IconKind::Svg => {
                let p = self.icon_path("scalable", stem, "svg");
                write_atomic(&p, &icon.bytes, 0o644).map_err(io_to_install)
            }
            IconKind::Png => {
                let Ok(img) =
                    image::load_from_memory_with_format(&icon.bytes, image::ImageFormat::Png)
                else {
                    // Bad icon is not worth failing an install for.
                    tracing::warn!("icon could not be decoded; installing without one");
                    return Ok(());
                };
                for size in icon_sizes(img.width().max(img.height())) {
                    let scaled = if img.width() == size && img.height() == size {
                        img.clone()
                    } else {
                        img.resize(size, size, image::imageops::FilterType::Lanczos3)
                    };
                    let mut buf = io::Cursor::new(Vec::new());
                    scaled
                        .write_to(&mut buf, image::ImageFormat::Png)
                        .map_err(|e| InstallError::Unknown(e.to_string()))?;
                    let p = self.icon_path(&format!("{size}x{size}"), stem, "png");
                    write_atomic(&p, &buf.into_inner(), 0o644).map_err(io_to_install)?;
                }
                Ok(())
            }
        }
    }

    fn remove_icons(&self, stem: &str) -> io::Result<()> {
        for size in ICON_SIZES {
            remove_if_exists(&self.icon_path(&format!("{size}x{size}"), stem, "png"))?;
        }
        remove_if_exists(&self.icon_path("scalable", stem, "svg"))
    }

    fn remove_blocking(&self, app: &InstalledApp) -> Result<(), InstallError> {
        let stem = desktop_stem(app.id.as_str());
        remove_if_exists(&self.applications_dir().join(format!("{stem}.desktop")))
            .and_then(|()| self.remove_icons(&stem))
            .map_err(io_to_install)
    }

    /// Best installed icon for `slug` (largest PNG, else SVG). Used by the ledger.
    pub fn load_icon(&self, slug: &str) -> Option<IconData> {
        let stem = desktop_stem(slug);
        for size in ICON_SIZES.iter().rev() {
            let p = self.icon_path(&format!("{size}x{size}"), &stem, "png");
            if let Ok(bytes) = fs::read(&p) {
                return Some(IconData {
                    kind: IconKind::Png,
                    bytes,
                });
            }
        }
        fs::read(self.icon_path("scalable", &stem, "svg"))
            .ok()
            .map(|bytes| IconData {
                kind: IconKind::Svg,
                bytes,
            })
    }
}

impl DesktopIntegration for DesktopEntries {
    async fn install_entry(
        &self,
        app: &InstalledApp,
        icon: Option<&IconData>,
    ) -> Result<String, InstallError> {
        let (this, app, icon) = (self.clone(), app.clone(), icon.cloned());
        tokio::task::spawn_blocking(move || this.write_blocking(&app, icon.as_ref()))
            .await
            .map_err(|e| InstallError::Unknown(e.to_string()))?
    }

    async fn remove_entry(&self, app: &InstalledApp) -> Result<(), InstallError> {
        let (this, app) = (self.clone(), app.clone());
        tokio::task::spawn_blocking(move || this.remove_blocking(&app))
            .await
            .map_err(|e| InstallError::Unknown(e.to_string()))?
    }
}

/// `$XDG_DATA_HOME` if absolute, else `$HOME/.local/share`.
pub fn default_data_home() -> Option<PathBuf> {
    if let Some(v) = std::env::var_os("XDG_DATA_HOME") {
        let p = PathBuf::from(v);
        if p.is_absolute() {
            return Some(p);
        }
    }
    std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .map(|h| PathBuf::from(h).join(".local/share"))
}

pub(crate) fn io_to_install(e: io::Error) -> InstallError {
    match e.kind() {
        io::ErrorKind::PermissionDenied => InstallError::PermissionDenied,
        io::ErrorKind::NotFound => InstallError::NotFound,
        io::ErrorKind::StorageFull | io::ErrorKind::QuotaExceeded => InstallError::NotEnoughSpace,
        _ => InstallError::Unknown(e.to_string()),
    }
}

fn remove_if_exists(p: &Path) -> io::Result<()> {
    match fs::remove_file(p) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// Write via a temp file in the same directory, fsync, rename.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8], mode: u32) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let dir = path
        .parent()
        .ok_or_else(|| io::Error::other("no parent dir"))?;
    fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(
        ".{}.tmp",
        path.file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default()
    ));
    let result = (|| {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.set_permissions(fs::Permissions::from_mode(mode))?;
        f.sync_all()?;
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use installer_domain::{AppId, AppSource, Version};

    use super::*;

    fn app(name: &str, path: &str) -> InstalledApp {
        InstalledApp {
            id: AppId::from_name(name),
            name: AppId::from_name(name).as_str().to_owned(),
            display_name: name.to_owned(),
            version: Version::new("1.0"),
            icon: None,
            source: AppSource::AppImage,
            installed_at: 0,
            desktop_id: None,
            path: Some(PathBuf::from(path)),
        }
    }

    fn png(side: u32) -> IconData {
        let img = image::RgbaImage::from_pixel(side, side, image::Rgba([1, 2, 3, 255]));
        let mut out = io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        IconData {
            kind: IconKind::Png,
            bytes: out.into_inner(),
        }
    }

    #[test]
    fn escape_value_table() {
        let rows = [
            ("plain", "plain"),
            ("a\\b", "a\\\\b"),
            ("line1\nline2", "line1\\nline2"),
            ("tab\there", "tab\\there"),
            ("cr\rx", "cr\\rx"),
            ("bell\u{7}x", "bellx"),
            ("Caf\u{e9}", "Caf\u{e9}"),
            ("", ""),
        ];
        for (input, want) in rows {
            assert_eq!(escape_value(input), want, "{input:?}");
        }
    }

    #[test]
    fn quote_exec_arg_table() {
        let rows = [
            (
                "/home/u/Applications/Lumen.AppImage",
                "\"/home/u/Applications/Lumen.AppImage\"",
            ),
            ("/home/u/My Apps/x", "\"/home/u/My Apps/x\""),
            ("/a\"b", "\"/a\\\"b\""),
            ("/a$b`c", "\"/a\\$b\\`c\""),
            ("/a\\b", "\"/a\\\\b\""),
            ("/100%/x", "\"/100%%/x\""),
        ];
        for (input, want) in rows {
            assert_eq!(quote_exec_arg(input), want, "{input:?}");
        }
    }

    #[test]
    fn exec_line_double_escapes_backslashes() {
        let text = render_entry("X", Path::new("/a$b"), "id");
        // quoting gives \$ ; file-level escaping doubles the backslash.
        assert!(text.contains("Exec=\"/a\\\\$b\" %U\n"), "{text}");
    }

    #[test]
    fn name_cannot_inject_keys() {
        let text = render_entry("Evil\nExec=/bin/sh", Path::new("/a/b.AppImage"), "id");
        assert_eq!(text.matches("\nExec=").count(), 1, "{text}");
        assert!(text.contains("Name=Evil\\nExec=/bin/sh\n"));
    }

    #[test]
    fn icon_sizes_table() {
        let rows: [(u32, &[u32]); 9] = [
            (16, &[48]),
            (47, &[48]),
            (48, &[48]),
            (64, &[48]),
            (100, &[48, 128]),
            (128, &[48, 128]),
            (200, &[48, 128, 256]),
            (256, &[48, 128, 256]),
            (1024, &[48, 128, 256]),
        ];
        for (side, want) in rows {
            assert_eq!(icon_sizes(side), want, "{side}");
        }
    }

    #[tokio::test]
    async fn install_and_remove_entry() {
        let dir = tempfile::tempdir().unwrap();
        let de = DesktopEntries::new(dir.path());
        let a = app("Lumen Notes", "/home/u/Applications/Lumen Notes.AppImage");
        let id = de.install_entry(&a, Some(&png(256))).await.unwrap();
        assert_eq!(id, "io.github.blueyzx64.Installer.lumen-notes.desktop");
        let file = dir.path().join("applications").join(&id);
        let text = fs::read_to_string(&file).unwrap();
        assert!(text.contains("Exec=\"/home/u/Applications/Lumen Notes.AppImage\" %U"));
        assert!(text.contains("X-Installer-Managed=true"));
        assert!(text.contains("Icon=io.github.blueyzx64.Installer.lumen-notes"));
        for size in ICON_SIZES {
            let p = dir.path().join(format!(
                "icons/hicolor/{size}x{size}/apps/io.github.blueyzx64.Installer.lumen-notes.png"
            ));
            let img = image::open(&p).unwrap();
            assert_eq!(img.width(), size);
        }
        assert_eq!(de.load_icon("lumen-notes").unwrap().kind, IconKind::Png);
        // No temp files are left behind.
        let leftovers: Vec<_> = fs::read_dir(dir.path().join("applications"))
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty());

        de.remove_entry(&a).await.unwrap();
        assert!(!file.exists());
        assert!(de.load_icon("lumen-notes").is_none());
        // Removing again is fine.
        de.remove_entry(&a).await.unwrap();
    }

    #[tokio::test]
    async fn svg_icon_goes_to_scalable() {
        let dir = tempfile::tempdir().unwrap();
        let de = DesktopEntries::new(dir.path());
        let a = app("Vec", "/home/u/Applications/Vec.AppImage");
        let icon = IconData {
            kind: IconKind::Svg,
            bytes: b"<svg/>".to_vec(),
        };
        de.install_entry(&a, Some(&icon)).await.unwrap();
        assert_eq!(de.load_icon("vec").unwrap().kind, IconKind::Svg);
    }

    #[tokio::test]
    async fn bad_png_installs_without_icon_and_relative_path_fails() {
        let dir = tempfile::tempdir().unwrap();
        let de = DesktopEntries::new(dir.path());
        let a = app("Bad", "/home/u/Applications/Bad.AppImage");
        let junk = IconData {
            kind: IconKind::Png,
            bytes: b"nope".to_vec(),
        };
        assert!(de.install_entry(&a, Some(&junk)).await.is_ok());
        assert!(de.load_icon("bad").is_none());
        let rel = app("Rel", "relative/x.AppImage");
        assert!(de.install_entry(&rel, None).await.is_err());
    }

    /// Validate with `desktop-file-validate` when it is installed (skipped otherwise).
    #[tokio::test]
    async fn generated_entry_validates() {
        let dir = tempfile::tempdir().unwrap();
        let de = DesktopEntries::new(dir.path());
        let a = app("Lumen Notes", "/home/u/Applications/Lumen Notes.AppImage");
        let id = de.install_entry(&a, None).await.unwrap();
        let file = dir.path().join("applications").join(id);
        match std::process::Command::new("desktop-file-validate")
            .arg(&file)
            .output()
        {
            Ok(out) => assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stdout)
            ),
            Err(_) => eprintln!("desktop-file-validate not installed; skipped"),
        }
    }
}
