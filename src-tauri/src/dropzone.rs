//! F-06: files dropped on a window become `app://open-file`, like single-instance.

use std::path::PathBuf;

use tauri::{DragDropEvent, Manager, WindowEvent};

use crate::cli::validate_package_path;
use crate::events;

/// Windows that accept drops.
pub const DROP_WINDOWS: &[&str] = &["main", "library"];

/// Keep only paths that are canonical regular files; drop the rest silently.
pub fn valid_drops(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .filter_map(|p| validate_package_path(&p.to_string_lossy()).ok())
        .map(|p| p.to_string_lossy().into_owned())
        .collect()
}

/// Window event hook: `.on_window_event(dropzone::on_window_event)`.
pub fn on_window_event(window: &tauri::Window, event: &WindowEvent) {
    let WindowEvent::DragDrop(DragDropEvent::Drop { paths, .. }) = event else {
        return;
    };
    if !DROP_WINDOWS.contains(&window.label()) {
        return;
    }
    let app = window.app_handle();
    for path in valid_drops(paths) {
        events::emit_open_file(app, &path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_files_drops_dirs_and_missing() {
        let dir = std::env::temp_dir().join("installer-dropzone-test");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.rpm");
        std::fs::write(&file, b"x").unwrap();
        let out = valid_drops(&[
            file.clone(),
            dir.clone(),
            dir.join("missing.rpm"),
            PathBuf::from("/dev/null"),
        ]);
        let want = file.canonicalize().unwrap().to_string_lossy().into_owned();
        assert_eq!(out, vec![want]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn symlink_is_canonicalised() {
        let dir = std::env::temp_dir().join("installer-dropzone-link");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("b.deb");
        let link = dir.join("link.deb");
        std::fs::write(&file, b"x").unwrap();
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink(&file, &link).unwrap();
        let out = valid_drops(&[link]);
        let want = file.canonicalize().unwrap().to_string_lossy().into_owned();
        assert_eq!(out, vec![want]);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
