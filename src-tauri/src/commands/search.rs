use tauri::State;

use crate::error::{AppError, AppResult};
use crate::models::search::SearchResult;
use crate::services::{dev_seed, search as search_service};
use crate::state::AppState;

/// Full-text search over entity names, summaries and lore (prefix matching,
/// best match first). Snippets mark matches with U+0002 / U+0003.
#[tauri::command]
#[specta::specta]
pub fn search(
    state: State<'_, AppState>,
    query: String,
    limit: u32,
) -> AppResult<Vec<SearchResult>> {
    state.with_world(|w| search_service::search(&w.conn, &query, limit))
}

/// Recovery tool: throws the index away and rebuilds it from the data.
#[tauri::command]
#[specta::specta]
pub fn rebuild_search_index(state: State<'_, AppState>) -> AppResult<()> {
    state.with_world(|w| search_service::rebuild(&w.conn))
}

/// Development only: generates `count` mixed entities with lore. Always
/// registered (so bindings are stable) but refuses to run in release builds.
#[tauri::command]
#[specta::specta]
pub fn dev_seed_entities(state: State<'_, AppState>, count: u32) -> AppResult<u32> {
    if !cfg!(debug_assertions) {
        return Err(AppError::Validation(
            "dev_seed_entities is only available in debug builds".into(),
        ));
    }
    state.with_world(|w| dev_seed::seed(&w.conn, count))
}
