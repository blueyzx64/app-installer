//! `install`, `cancel_install`.

use tauri::State;
use tauri::ipc::Channel;

use crate::dto::{InstallEvent, InstallOutcome};
use crate::error::CommandError;
use crate::events::ChannelObserver;
use crate::state::AppState;

/// Streams progress over `on_event`; resolves when finished.
/// Failures and cancels are outcomes, not errors.
#[tauri::command]
#[specta::specta]
pub async fn install(
    state: State<'_, AppState>,
    preview_id: String,
    on_event: Channel<InstallEvent>,
) -> Result<InstallOutcome, CommandError> {
    let id = preview_id.parse()?;
    tracing::info!(preview = %preview_id, "install start");
    let observer = ChannelObserver::new(on_event);
    let outcome: InstallOutcome = state.install_package.execute(id, &observer).await.into();
    match &outcome {
        InstallOutcome::Installed { app } => {
            tracing::info!(app = %app.display_name, "install installed")
        }
        InstallOutcome::Cancelled => tracing::info!("install cancelled"),
        InstallOutcome::Failed { code, .. } => tracing::info!(code = ?code, "install failed"),
    }
    Ok(outcome)
}

/// Best effort. A preview that is not installing is ignored.
#[tauri::command]
#[specta::specta]
pub async fn cancel_install(
    state: State<'_, AppState>,
    preview_id: String,
) -> Result<(), CommandError> {
    tracing::info!(preview = %preview_id, "cancel_install");
    state.cancel_install.execute(preview_id.parse()?);
    Ok(())
}
