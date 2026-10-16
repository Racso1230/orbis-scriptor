use tauri::State;

use crate::error::AppResult;
use crate::models::entity::{Entity, EntityPatch, EntitySummary, NewEntity};
use crate::models::entity_type::{self, EntityTypeDef};
use crate::services::entities;
use crate::state::AppState;

/// The built-in entity types with their fields (the sidebar and forms are
/// driven by this).
#[tauri::command]
#[specta::specta]
pub fn entity_types() -> Vec<EntityTypeDef> {
    entity_type::all()
}

/// Lists entities of one type, or all of them when `entity_type` is null.
#[tauri::command]
#[specta::specta]
pub fn list_entities(
    state: State<'_, AppState>,
    entity_type: Option<String>,
) -> AppResult<Vec<EntitySummary>> {
    state.with_world(|w| entities::list(&w.conn, entity_type.as_deref()))
}

#[tauri::command]
#[specta::specta]
pub fn get_entity(state: State<'_, AppState>, id: String) -> AppResult<Entity> {
    state.with_world(|w| entities::get(&w.conn, &id))
}

#[tauri::command]
#[specta::specta]
pub fn create_entity(state: State<'_, AppState>, input: NewEntity) -> AppResult<Entity> {
    state.with_world(|w| entities::create(&w.conn, input))
}

#[tauri::command]
#[specta::specta]
pub fn update_entity(
    state: State<'_, AppState>,
    id: String,
    patch: EntityPatch,
) -> AppResult<Entity> {
    state.with_world(|w| entities::update(&w.conn, &id, patch))
}

#[tauri::command]
#[specta::specta]
pub fn delete_entity(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.with_world(|w| entities::delete(&w.conn, &id))
}
