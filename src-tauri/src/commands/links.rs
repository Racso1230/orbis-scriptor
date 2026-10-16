use tauri::State;

use crate::error::AppResult;
use crate::models::link::Backlink;
use crate::services::{entities, links};
use crate::state::AppState;

/// Entities whose lore mentions `entity_id`.
#[tauri::command]
#[specta::specta]
pub fn get_backlinks(state: State<'_, AppState>, entity_id: String) -> AppResult<Vec<Backlink>> {
    state.with_world(|w| {
        let target = entities::get(&w.conn, &entity_id)?;
        links::backlinks(&w.conn, &entity_id, &target.name)
    })
}
