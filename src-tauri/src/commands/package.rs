//! `open_package`, `take_launch_file`.

use tauri::State;

use crate::error::CommandError;
use crate::state::AppState;

/// The path from argv on first launch. Consumed once.
#[tauri::command]
#[specta::specta]
pub async fn take_launch_file(state: State<'_, AppState>) -> Result<Option<String>, CommandError> {
    Ok(state
        .take_launch_file()
        .map(|p| p.to_string_lossy().into_owned()))
}
