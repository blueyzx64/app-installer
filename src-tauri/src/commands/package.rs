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
    tracing::info!("open_package start");
    let path = cli::validate_package_path(&path).inspect_err(log_rejected)?;
    match state.open_package.execute(path).await {
        Ok(preview) => {
            let preview: PackagePreview = preview.into();
            tracing::info!(format = ?preview.format, name = %preview.name, "open_package ok");
            Ok(preview)
        }
        Err(e) => {
            let e = CommandError::from(e);
            log_rejected(&e);
            Err(e)
        }
    }
}

fn log_rejected(e: &CommandError) {
    tracing::info!(code = ?e.code, "open_package failed");
}
