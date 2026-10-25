//! `pick_package` (F-07): GTK / portal file chooser, filtered to app packages.

use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

use crate::cli::validate_package_path;
use crate::error::{CommandError, ErrorCode};

/// Extensions shown by the chooser (portals can be case-sensitive, so list both spellings).
pub const PACKAGE_EXTENSIONS: &[&str] = &["rpm", "AppImage", "appimage", "deb"];

/// Opens the chooser. `None` means the person dismissed it (a cancel, not an error).
#[tauri::command]
#[specta::specta]
pub async fn pick_package(app: AppHandle) -> Result<Option<String>, CommandError> {
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .add_filter("App packages", PACKAGE_EXTENSIONS)
            .blocking_pick_file()
    })
    .await
    .map_err(|e| CommandError::new(ErrorCode::Unknown, e.to_string()))?;
    let Some(picked) = picked else {
        return Ok(None);
    };
    let path = picked
        .into_path()
        .map_err(|e| CommandError::new(ErrorCode::NotFound, e.to_string()))?;
    let canonical = validate_package_path(&path.to_string_lossy())?;
    Ok(Some(canonical.to_string_lossy().into_owned()))
}
