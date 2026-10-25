//! Composition root: the only place that names concrete adapter types.
//!
//! W0a stub. Later it builds the infrastructure adapters, injects them into
//! the use cases and returns a fully wired `AppState`.

use std::path::PathBuf;

use crate::state::AppState;

pub fn build_state(launch_file: Option<PathBuf>) -> AppState {
    AppState::new(launch_file)
}
