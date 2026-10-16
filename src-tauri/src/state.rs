//! Application state that Tauri keeps alive for the whole process.
//!
//! Tauri "managed state" is registered once with `.manage(...)` and any
//! command can then ask for it as a `tauri::State<'_, AppState>` argument.

use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::Connection;

use crate::models::world::{WorldManifest, WorldSummary};

/// Everything that belongs to the currently open world.
///
/// Dropping this struct closes the database connection, so "close world" is
/// simply `*state.world = None`.
pub struct WorldHandle {
    /// Absolute path of the world folder (`PathBuf` = owned path).
    pub path: PathBuf,
    pub manifest: WorldManifest,
    pub conn: Connection,
}

impl WorldHandle {
    pub fn summary(&self) -> WorldSummary {
        WorldSummary {
            name: self.manifest.name.clone(),
            path: self.path.to_string_lossy().into_owned(),
        }
    }
}

/// Shared state for the whole app.
///
/// Commands may run on different threads at the same time, so the open world
/// sits behind a `Mutex`: a command locks it, uses it, and the lock is released
/// when the guard goes out of scope. `Option` models "no world open yet".
#[derive(Default)]
pub struct AppState {
    pub world: Mutex<Option<WorldHandle>>,
}
