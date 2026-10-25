//! tauri-specta wiring (ADR-0009). One builder serves both the runtime
//! `invoke_handler` and the TypeScript export.

use std::path::Path;

use specta_typescript::Typescript;
use tauri_specta::{Builder, ErrorHandlingMode, collect_commands};

use crate::commands::{appearance, defaults, files, install, library, package};

/// Where the generated file is committed, relative to `src-tauri/`.
pub const BINDINGS_PATH: &str = "../src/lib/infrastructure/tauri/bindings.ts";

pub fn builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        // Commands reject with `CommandError`; wrappers are plain `Promise<T>`.
        .error_handling(ErrorHandlingMode::Throw)
        // One TS type per Rust type (no `_Serialize` / `_Deserialize` pairs).
        .disable_serde_phases()
        .commands(collect_commands![
            package::take_launch_file,
            package::open_package,
            install::install,
            install::cancel_install,
            library::list_installed,
            library::uninstall,
            library::launch_app,
            library::uninstall_plan,
            defaults::default_handler_status,
            defaults::make_default_handler,
            defaults::dismiss_default_prompt,
            appearance::get_appearance,
            files::pick_package, // lane S4b
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
    fn export_has_every_command_with_contract_type_names() {
        let path = std::env::temp_dir().join("installer-bindings-test.ts");
        export(&path).unwrap();
        let ts = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(path);
        for cmd in [
            "takeLaunchFile",
            "openPackage",
            "install:",
            "cancelInstall",
            "listInstalled",
            "uninstall:",
            "launchApp",
            "uninstallPlan",
            "defaultHandlerStatus",
            "makeDefaultHandler",
            "dismissDefaultPrompt",
            "getAppearance",
            "pickPackage",
        ] {
            assert!(ts.contains(cmd), "missing command {cmd}");
        }
        for ty in [
            "PackagePreview",
            "Compatibility",
            "InstallPhase",
            "InstallEvent",
            "InstallOutcome",
            "InstalledApp",
            "ErrorCode",
            "CommandError",
            "Appearance",
            "DefaultStatus",
            "UninstallOutcome",
            "UninstallPlan",
        ] {
            assert!(
                ts.contains(&format!("export type {ty} =")),
                "missing type {ty}"
            );
        }
        assert!(ts.contains("onEvent: Channel<InstallEvent>"));
        assert!(!ts.contains("_Serialize"));
    }
}
