//! `list_installed`, `uninstall`, `launch_app`.

use tauri::State;
use tauri::ipc::Channel;

use crate::dto::{InstallEvent, InstalledApp, UninstallOutcome, UninstallPlan};
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
    tracing::info!(app = %app_id, "uninstall start");
    let observer = ChannelObserver::new(on_event);
    let outcome: UninstallOutcome = state.uninstall_app.execute(&id, &observer).await.into();
    match &outcome {
        UninstallOutcome::Removed => tracing::info!("uninstall removed"),
        UninstallOutcome::Cancelled => tracing::info!("uninstall cancelled"),
        UninstallOutcome::Failed { code, .. } => tracing::info!(code = ?code, "uninstall failed"),
    }
    Ok(outcome)
}

#[tauri::command]
#[specta::specta]
pub async fn launch_app(state: State<'_, AppState>, app_id: String) -> Result<(), CommandError> {
    tracing::info!(app = %app_id, "launch_app");
    let result = state.launch_app.execute(&app_id.parse()?).await;
    if let Err(e) = &result {
        tracing::info!(error = %e, "launch_app failed");
    }
    Ok(result?)
}

/// What removing the app would do. Asked before the confirm sheet (F-42).
#[tauri::command]
#[specta::specta]
pub async fn uninstall_plan(
    state: State<'_, AppState>,
    app_id: String,
) -> Result<UninstallPlan, CommandError> {
    let id = app_id.parse()?;
    Ok(state.uninstall_plan.execute(&id).await?.into())
}
