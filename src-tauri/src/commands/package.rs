//! `take_launch_file`, `open_package`.

use tauri::State;

use crate::cli;
use crate::dto::PackagePreview;
use crate::error::CommandError;
use crate::state::AppState;

/// The path from argv on first launch. Consumed once.
#[tauri::command]
#[specta::specta]
pub async fn take_launch_file(state: State<'_, AppState>) -> Result<Option<String>, CommandError> {
    Ok(state
        .launch_file
        .take()
        .map(|p| p.to_string_lossy().into_owned()))
}

/// Parses only. The preview is stored server-side under its `id`.
#[tauri::command]
#[specta::specta]
pub async fn open_package(
    state: State<'_, AppState>,
    path: String,
) -> Result<PackagePreview, CommandError> {
    let path = cli::validate_package_path(&path)?;
    Ok(state.open_package.execute(path).await?.into())
}
