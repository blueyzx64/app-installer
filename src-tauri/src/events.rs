//! Rust -> frontend notifications: the global `app://open-file` and
//! `appearance://changed` events, and the per-call install progress channel.

use installer_application::ports::{AppearanceObserver, InstallObserver};
use installer_domain as domain;
use serde::Serialize;
use specta::Type;
use tauri::ipc::Channel;
use tauri::{AppHandle, Emitter};

use crate::dto::{Appearance, InstallEvent};

pub const OPEN_FILE_EVENT: &str = "app://open-file";
pub const APPEARANCE_CHANGED_EVENT: &str = "appearance://changed";

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

/// Forwards install progress to one `Channel`.
pub struct ChannelObserver {
    channel: Channel<InstallEvent>,
}

impl ChannelObserver {
    pub fn new(channel: Channel<InstallEvent>) -> Self {
        Self { channel }
    }

    /// Send errors are ignored: the window may have closed.
    fn send(&self, event: InstallEvent) {
        if let Err(err) = self.channel.send(event) {
            log::debug!("install channel closed: {err}");
        }
    }
}

impl InstallObserver for ChannelObserver {
    fn on_progress(&self, progress: domain::InstallProgress) {
        self.send(progress.into());
    }

    fn on_plan(&self, plan: &domain::InstallPlan) {
        self.send(plan.into());
    }
}

/// Re-emits appearance changes as the global `appearance://changed` event.
pub struct AppearanceEmitter {
    app: AppHandle,
}

impl AppearanceEmitter {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl AppearanceObserver for AppearanceEmitter {
    fn on_change(&self, appearance: domain::Appearance) {
        if let Err(err) = self
            .app
            .emit(APPEARANCE_CHANGED_EVENT, Appearance::from(appearance))
        {
            log::warn!("could not emit {APPEARANCE_CHANGED_EVENT}: {err}");
        }
    }
}
