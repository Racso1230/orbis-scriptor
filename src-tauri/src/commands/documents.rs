use tauri::State;

use crate::error::AppResult;
use crate::models::document::Document;
use crate::services::documents;
use crate::state::AppState;

/// The lore document of an entity (empty document if none was saved yet).
#[tauri::command]
#[specta::specta]
pub fn get_document(state: State<'_, AppState>, entity_id: String) -> AppResult<Document> {
    state.with_world(|w| documents::get(&w.conn, &entity_id))
}

/// Saves the editor's JSON plus the plain text used for search.
#[tauri::command]
#[specta::specta]
pub fn save_document(
    state: State<'_, AppState>,
    entity_id: String,
    content: String,
    plain_text: String,
) -> AppResult<Document> {
    state.with_world(|w| documents::save(&w.conn, &entity_id, content, plain_text))
}
