//! Orbis Scriptor backend. `main.rs` is a thin wrapper that calls [`run`].

/// Builds and starts the Tauri application.
///
/// `expect` is allowed here because this is startup code: if the app cannot
/// start there is nothing sensible to do except exit with the error.
#[allow(clippy::expect_used)]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
