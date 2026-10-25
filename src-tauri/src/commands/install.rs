//! `install`, `cancel_install`.

use tauri::ipc::Channel;

use crate::dto::{InstallEventDto, InstallOutcomeDto, InstallPhaseDto};
use crate::error::CommandError;
use crate::events::ChannelObserver;

/// SPIKE STUB (ADR-0009): proves a `Channel` argument survives code generation.
/// Replaced by the real use case call when the application crate lands.
#[tauri::command]
#[specta::specta]
pub async fn install(
    preview_id: String,
    on_event: Channel<InstallEventDto>,
) -> Result<InstallOutcomeDto, CommandError> {
    let observer = ChannelObserver::new(on_event);
    log::debug!("install stub for preview {preview_id}");
    observer.send(InstallEventDto::Progress {
        phase: InstallPhaseDto::Preparing,
        percent: None,
        eta_secs: None,
    });
    Ok(InstallOutcomeDto::Cancelled)
}
