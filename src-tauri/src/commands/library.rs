//! `list_installed`, `uninstall`, `launch_app`.

use tauri::State;
use tauri::ipc::Channel;

use crate::dto::{InstallEvent, InstalledApp, UninstallOutcome};
use crate::error::CommandError;
use crate::events::ChannelObserver;
use crate::state::AppState;

#[tauri::command]
#[specta::specta]
pub async fn list_installed(state: State<'_, AppState>) -> Result<Vec<InstalledApp>, CommandError> {
    let apps = state.list_installed.execute().await?;
    Ok(apps.into_iter().map(Into::into).collect())
}

/// System packages prompt through polkit; cancelling is an outcome.
#[tauri::command]
#[specta::specta]
pub async fn uninstall(
    state: State<'_, AppState>,
    app_id: String,
    on_event: Channel<InstallEvent>,
) -> Result<UninstallOutcome, CommandError> {
    let id = app_id.parse()?;
    let observer = ChannelObserver::new(on_event);
    Ok(state.uninstall_app.execute(&id, &observer).await.into())
}

#[tauri::command]
#[specta::specta]
pub async fn launch_app(state: State<'_, AppState>, app_id: String) -> Result<(), CommandError> {
    Ok(state.launch_app.execute(&app_id.parse()?).await?)
}
