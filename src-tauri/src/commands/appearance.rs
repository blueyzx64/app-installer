//! `get_appearance`. Changes arrive through the `appearance://changed` event.

use tauri::State;

use crate::dto::Appearance;
use crate::error::CommandError;
use crate::state::AppState;

#[tauri::command]
#[specta::specta]
pub async fn get_appearance(state: State<'_, AppState>) -> Result<Appearance, CommandError> {
    Ok(state.get_appearance.execute().await.into())
}
