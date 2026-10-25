//! Tauri shell: interface adapters and composition root (ARCHITECTURE §8).

pub mod bindings;
pub mod cli;
pub mod commands;
pub mod composition;
pub mod dto;
pub mod error;
pub mod events;
pub mod logging; // lane S11
pub mod state;

use tauri::Manager;

pub fn run() {
    logging::init(); // lane S11
    let launch_file = cli::file_from_argv(&std::env::args().collect::<Vec<_>>());
    let specta = bindings::builder();

    tauri::Builder::default()
        // Must be registered first (single-instance requirement).
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if let Some(path) = cli::file_from_argv(&argv) {
                events::emit_open_file(app, &path.to_string_lossy());
            }
            if let Some(window) = app.get_webview_window("main") {
                if let Err(err) = window.set_focus() {
                    log::debug!("could not focus main window: {err}");
                }
            }
        }))
        .plugin(tauri_plugin_log::Builder::new().build())
        .manage(composition::build_state(launch_file))
        .invoke_handler(specta.invoke_handler())
        .setup(move |app| {
            specta.mount_events(app);
            let observer =
                std::sync::Arc::new(events::AppearanceEmitter::new(app.handle().clone()));
            app.state::<state::AppState>()
                .watch_appearance
                .execute(observer);
            Ok(())
        })
        .run(tauri::generate_context!())
        // Startup failure is unrecoverable; Tauri documents this as the entry-point idiom.
        .expect("error while running tauri application");
}
