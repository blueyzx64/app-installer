//! `default_handler_status`, `make_default_handler`, `dismiss_default_prompt`.

use tauri::State;

use crate::dto::DefaultStatus;
use crate::error::CommandError;
use crate::state::AppState;

#[tauri::command]
#[specta::specta]
pub async fn default_handler_status(
    state: State<'_, AppState>,
) -> Result<DefaultStatus, CommandError> {
    Ok(state.default_status.execute().await?.into())
}

/// Makes this app the default for all supported MIME types.
#[tauri::command]
#[specta::specta]
pub async fn make_default_handler(state: State<'_, AppState>) -> Result<(), CommandError> {
    let result = state.make_default.execute().await;
    tracing::info!(ok = result.is_ok(), "make_default_handler");
    Ok(result?)
}

/// Remembers "Not Now".
#[tauri::command]
#[specta::specta]
pub async fn dismiss_default_prompt(state: State<'_, AppState>) -> Result<(), CommandError> {
    tracing::info!("dismiss_default_prompt");
    Ok(state.dismiss_prompt.execute().await?)
}
