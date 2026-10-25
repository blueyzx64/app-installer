//! Shared app state, managed by Tauri.
//!
//! Will grow to hold the use cases, the `PreviewRegistry` and active install
//! handles once `composition.rs` wires the real adapters.

use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Default)]
pub struct AppState {
    launch_file: Mutex<Option<PathBuf>>,
}

impl AppState {
    pub fn new(launch_file: Option<PathBuf>) -> Self {
        Self {
            launch_file: Mutex::new(launch_file),
        }
    }

    /// Returns the file from the first launch's argv, once. Later calls give `None`.
    pub fn take_launch_file(&self) -> Option<PathBuf> {
        // A poisoned lock only means another thread panicked; the data is still valid.
        let mut guard = self
            .launch_file
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_file_is_taken_once() {
        let state = AppState::new(Some(PathBuf::from("/tmp/a.rpm")));
        assert_eq!(state.take_launch_file(), Some(PathBuf::from("/tmp/a.rpm")));
        assert_eq!(state.take_launch_file(), None);
    }
}
