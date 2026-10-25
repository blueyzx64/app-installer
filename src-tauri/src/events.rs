//! Rust -> frontend notifications: the global `app://open-file` event and the
//! per-call install progress channel.

use serde::Serialize;
use specta::Type;
use tauri::ipc::Channel;
use tauri::{AppHandle, Emitter};

use crate::dto::InstallEventDto;

pub const OPEN_FILE_EVENT: &str = "app://open-file";

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OpenFilePayload {
    pub path: String,
}

pub fn emit_open_file(app: &AppHandle, path: &str) {
    let payload = OpenFilePayload {
        path: path.to_owned(),
    };
    if let Err(err) = app.emit(OPEN_FILE_EVENT, payload) {
        log::warn!("could not emit {OPEN_FILE_EVENT}: {err}");
    }
}

/// Forwards install progress to one `Channel`. Will implement the application
/// layer's `InstallObserver` once S1's ports are merged.
pub struct ChannelObserver {
    channel: Channel<InstallEventDto>,
}

impl ChannelObserver {
    pub fn new(channel: Channel<InstallEventDto>) -> Self {
        Self { channel }
    }

    /// Send errors are ignored: the window may have closed.
    pub fn send(&self, event: InstallEventDto) {
        if let Err(err) = self.channel.send(event) {
            log::debug!("install channel closed: {err}");
        }
    }
}
