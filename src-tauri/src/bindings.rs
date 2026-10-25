//! tauri-specta wiring (ADR-0009). One builder serves both the runtime
//! `invoke_handler` and the TypeScript export.

use std::path::Path;

use specta_typescript::Typescript;
use tauri_specta::{Builder, collect_commands};

use crate::commands::{install, package};

/// Where the generated file is committed, relative to `src-tauri/`.
pub const BINDINGS_PATH: &str = "../src/lib/infrastructure/tauri/bindings.ts";

pub fn builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new().commands(collect_commands![
        package::take_launch_file,
        install::install,
    ])
}

pub fn export(path: impl AsRef<Path>) -> anyhow::Result<()> {
    builder().export(Typescript::default(), path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_contains_commands_and_channel() {
        let dir = std::env::temp_dir().join("installer-bindings-test.ts");
        export(&dir).unwrap();
        let ts = std::fs::read_to_string(&dir).unwrap();
        assert!(ts.contains("takeLaunchFile"));
        assert!(ts.contains("onEvent: Channel<InstallEventDto"));
        let _ = std::fs::remove_file(dir);
    }
}
