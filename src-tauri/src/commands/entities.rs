use tauri::State;

use crate::error::AppResult;
use crate::models::entity::{Entity, EntityPatch, EntitySummary, NewEntity};
use crate::models::entity_type::EntityTypeDef;
use crate::services::{entities, entity_types, gallery, world};
use crate::state::AppState;

/// All entity types of the open world with their fields: the built-in
/// types (with any extra user fields), then the custom types. The sidebar
/// and forms are driven by this.
#[tauri::command]
#[specta::specta]
pub fn entity_types(state: State<'_, AppState>) -> AppResult<Vec<EntityTypeDef>> {
    state.with_world(|w| entity_types::merged(&w.conn))
}

/// Lists entities of one type, or all of them when `entity_type` is null.
#[tauri::command]
#[specta::specta]
pub fn list_entities(
    state: State<'_, AppState>,
    entity_type: Option<String>,
) -> AppResult<Vec<EntitySummary>> {
    state.with_world(|w| {
        let mut items = entities::list(&w.conn, entity_type.as_deref())?;
        gallery::attach_portraits(&w.conn, &w.path.join(world::ASSETS_DIR), &mut items)?;
        Ok(items)
    })
}

/// One entity with its tags, favourite flag and portrait.
#[tauri::command]
#[specta::specta]
pub fn get_entity(state: State<'_, AppState>, id: String) -> AppResult<Entity> {
    state.with_world(|w| {
        let mut entity = entities::get(&w.conn, &id)?;
        gallery::attach_portrait(&w.conn, &w.path.join(world::ASSETS_DIR), &mut entity)?;
        Ok(entity)
    })
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
    state.with_world(|w| {
        let mut entity = entities::update(&w.conn, &id, patch)?;
        gallery::attach_portrait(&w.conn, &w.path.join(world::ASSETS_DIR), &mut entity)?;
        Ok(entity)
    })
}

#[tauri::command]
#[specta::specta]
pub fn delete_entity(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state
        .with_world(|w| entities::delete_with_assets(&w.conn, &w.path.join(world::ASSETS_DIR), &id))
}

/// Stars (`true`) or un-stars an entity. Not an edit: `updatedAt` stays.
#[tauri::command]
#[specta::specta]
pub fn set_favorite(
    state: State<'_, AppState>,
    entity_id: String,
    favorite: bool,
) -> AppResult<()> {
    state.with_world(|w| entities::set_favorite(&w.conn, &entity_id, favorite))
}

/// Starred entities, sorted by type then name.
#[tauri::command]
#[specta::specta]
pub fn list_favorites(state: State<'_, AppState>) -> AppResult<Vec<EntitySummary>> {
    state.with_world(|w| {
        let mut items = entities::list_favorites(&w.conn)?;
        gallery::attach_portraits(&w.conn, &w.path.join(world::ASSETS_DIR), &mut items)?;
        Ok(items)
    })
}

/// The most recently edited entities, newest first (`limit` 1 to 100).
/// Entity edits, lore saves, event date changes and tag changes count.
#[tauri::command]
#[specta::specta]
pub fn list_recent(state: State<'_, AppState>, limit: u32) -> AppResult<Vec<EntitySummary>> {
    state.with_world(|w| {
        let mut items = entities::list_recent(&w.conn, limit)?;
        gallery::attach_portraits(&w.conn, &w.path.join(world::ASSETS_DIR), &mut items)?;
        Ok(items)
    })
}
