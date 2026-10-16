//! Application state that Tauri keeps alive for the whole process.
//!
//! Tauri "managed state" is registered once with `.manage(...)` and any
//! command can then ask for it as a `tauri::State<'_, AppState>` argument.

use std::sync::Mutex;

/// Everything that belongs to the currently open world.
/// Phase 1 fills this in (database connection, folder path, ...).
pub struct WorldHandle;

/// Shared state for the whole app.
///
/// Commands may run on different threads at the same time, so the open world
/// sits behind a `Mutex`: a command locks it, uses it, and the lock is released
/// when the guard goes out of scope. `Option` models "no world open yet".
#[derive(Default)]
pub struct AppState {
    pub world: Mutex<Option<WorldHandle>>,
}
