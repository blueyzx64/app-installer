//! Tauri shell: interface adapters and composition root (ARCHITECTURE §8).

pub mod bindings;
pub mod cli;
pub mod commands;
pub mod composition;
pub mod dropzone; // lane S4b
pub mod dto;
pub mod error;
pub mod events;
pub mod fallbacks;
pub mod logging; // lane S11
pub mod state;
pub mod system_backend;

use tauri::Manager;

pub fn run() {
    logging::init(); // lane S11
    let launch_file = std::env::current_dir()
        .ok()
        .and_then(|cwd| cli::launch_path_from_argv(&std::env::args_os().collect::<Vec<_>>(), &cwd));
    let specta = bindings::builder();

    tauri::Builder::default()
        // Must be registered first (single-instance requirement).
        .plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            if let Some(payload) = cli::open_file_payload(&argv, std::path::Path::new(&cwd)) {
                events::emit_open_file(app, &payload.path);
            }
            if let Some(window) = app.get_webview_window("main") {
                if let Err(err) = window.set_focus() {
                    log::debug!("could not focus main window: {err}");
                }
            }
        }))
        .plugin(tauri_plugin_dialog::init()) // lane S4b
        .on_window_event(dropzone::on_window_event) // lane S4b
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
