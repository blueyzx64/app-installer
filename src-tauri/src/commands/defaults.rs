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
    Ok(state.make_default.execute().await?)
}

/// Remembers "Not Now".
#[tauri::command]
#[specta::specta]
pub async fn dismiss_default_prompt(state: State<'_, AppState>) -> Result<(), CommandError> {
    Ok(state.dismiss_prompt.execute().await?)
}
