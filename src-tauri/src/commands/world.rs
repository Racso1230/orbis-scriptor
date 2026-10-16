use std::path::{Path, PathBuf};
use std::sync::MutexGuard;

use tauri::{AppHandle, Manager, State};

use crate::error::{AppError, AppResult};
use crate::models::world::{RecentWorld, WorldSummary};
use crate::services::{recent_worlds, world};
use crate::state::{AppState, WorldHandle};

/// Where the recent-worlds file lives: the per-user app config directory
/// (e.g. `%APPDATA%\com.racso1230.orbisscriptor` on Windows).
fn recent_file(app: &AppHandle) -> AppResult<PathBuf> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| AppError::Internal(format!("cannot resolve config dir: {e}")))?;
    Ok(dir.join("recent-worlds.json"))
}

/// Locks the open-world slot.
///
/// `MutexGuard` is a smart pointer: while it exists nobody else can lock the
/// mutex, and the lock is released automatically when it goes out of scope.
/// `lock()` only fails if another thread panicked while holding the lock
/// ("poisoned"), which we report instead of unwrapping.
fn lock_world<'r>(state: &State<'r, AppState>) -> AppResult<MutexGuard<'r, Option<WorldHandle>>> {
    state
        .inner()
        .world
        .lock()
        .map_err(|_| AppError::Internal("world state lock is poisoned".into()))
}

/// Puts a freshly created/opened world into state and records it as recent.
fn install(
    app: &AppHandle,
    state: &State<'_, AppState>,
    handle: WorldHandle,
) -> AppResult<WorldSummary> {
    let summary = handle.summary();
    recent_worlds::remember(&recent_file(app)?, &summary)?;
    *lock_world(state)? = Some(handle);
    Ok(summary)
}

/// Creates a new world folder and opens it.
#[tauri::command]
#[specta::specta]
pub fn create_world(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    name: String,
) -> AppResult<WorldSummary> {
    let handle = world::create_world(Path::new(&path), &name)?;
    install(&app, &state, handle)
}

/// Opens an existing world folder (running migrations if needed).
#[tauri::command]
#[specta::specta]
pub fn open_world(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> AppResult<WorldSummary> {
    let handle = world::open_world(Path::new(&path))?;
    install(&app, &state, handle)
}

/// Closes the open world, if any. Dropping the handle closes the database.
#[tauri::command]
#[specta::specta]
pub fn close_world(state: State<'_, AppState>) -> AppResult<()> {
    *lock_world(&state)? = None;
    Ok(())
}

/// The currently open world, or `null`.
#[tauri::command]
#[specta::specta]
pub fn current_world(state: State<'_, AppState>) -> AppResult<Option<WorldSummary>> {
    Ok(lock_world(&state)?.as_ref().map(WorldHandle::summary))
}

#[tauri::command]
#[specta::specta]
pub fn list_recent_worlds(app: AppHandle) -> AppResult<Vec<RecentWorld>> {
    Ok(recent_worlds::list(&recent_file(&app)?))
}

#[tauri::command]
#[specta::specta]
pub fn remove_recent_world(app: AppHandle, path: String) -> AppResult<()> {
    recent_worlds::forget(&recent_file(&app)?, &path)
}
