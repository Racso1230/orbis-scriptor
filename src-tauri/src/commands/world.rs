use std::path::{Path, PathBuf};
use std::sync::MutexGuard;

use tauri::{AppHandle, Manager, State};

use crate::error::{AppError, AppResult};
use crate::models::world::{RecentWorld, WorldSummary};
use crate::services::{backups, recent_worlds, sample_world, world};
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

/// Lets the webview load images from the open world's `assets/` folder (and
/// nothing else) through Tauri's asset protocol. The static scope in
/// `tauri.conf.json` is empty; this adds the folder at runtime and removes
/// the previous world's folder, so only one world's files are reachable.
fn switch_asset_scope(app: &AppHandle, old: Option<&Path>, new: Option<&Path>) {
    let scope = app.asset_protocol_scope();
    // Re-opening the same world: nothing to change (revoking would undo the grant).
    if old == new {
        return;
    }
    // Failures here only affect image display, so they are logged, not fatal.
    if let Some(dir) = old
        && let Err(e) = scope.forbid_directory(dir.join(world::ASSETS_DIR), false)
    {
        eprintln!("could not revoke asset scope: {e}");
    }
    if let Some(dir) = new
        && let Err(e) = scope.allow_directory(dir.join(world::ASSETS_DIR), false)
    {
        eprintln!("could not grant asset scope: {e}");
    }
}

/// Puts a freshly created/opened world into state and records it as recent.
fn install(
    app: &AppHandle,
    state: &State<'_, AppState>,
    handle: WorldHandle,
) -> AppResult<WorldSummary> {
    let summary = handle.summary();
    recent_worlds::remember(&recent_file(app)?, &summary)?;
    // `replace` swaps in the new world and hands back the previous one (which
    // is dropped at the end, closing its database).
    let new_path = handle.path.clone();
    let previous = lock_world(state)?.replace(handle);
    switch_asset_scope(
        app,
        previous.as_ref().map(|w| w.path.as_path()),
        Some(&new_path),
    );
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

/// Creates the sample world "The Shattered Coast" in a new folder inside
/// `parent_dir`, fills it with linked entities, a map and events, and opens it.
#[tauri::command]
#[specta::specta]
pub fn create_sample_world(
    app: AppHandle,
    state: State<'_, AppState>,
    parent_dir: String,
) -> AppResult<WorldSummary> {
    let handle = sample_world::create(Path::new(&parent_dir))?;
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
    let world_dir = handle.path.clone();
    let summary = install(&app, &state, handle)?;
    // Daily backup runs in the background; it never blocks or fails the open.
    backups::spawn_daily(world_dir);
    Ok(summary)
}

/// Closes the open world, if any. Dropping the handle closes the database.
#[tauri::command]
#[specta::specta]
pub fn close_world(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    let previous = lock_world(&state)?.take();
    switch_asset_scope(&app, previous.as_ref().map(|w| w.path.as_path()), None);
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
