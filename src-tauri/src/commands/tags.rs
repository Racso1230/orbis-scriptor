use tauri::State;

use crate::error::AppResult;
use crate::models::entity::EntitySummary;
use crate::models::tag::Tag;
use crate::services::tags;
use crate::state::AppState;

/// Every tag with its usage count, sorted by name.
#[tauri::command]
#[specta::specta]
pub fn list_tags(state: State<'_, AppState>) -> AppResult<Vec<Tag>> {
    state.with_world(|w| tags::list(&w.conn))
}

/// Replaces the tags of an entity with `names` (missing tags are created,
/// names that differ only in case are one tag). Returns the entity's tag
/// names. Names: 1 to 40 characters of letters, digits, `-`, `_`, spaces
/// and `/` (nested tags such as `places/cities`).
#[tauri::command]
#[specta::specta]
pub fn set_entity_tags(
    state: State<'_, AppState>,
    entity_id: String,
    names: Vec<String>,
) -> AppResult<Vec<String>> {
    state.with_world(|w| tags::set_entity_tags(&w.conn, &entity_id, &names))
}

/// Renames a tag. If another tag already has the new name (ignoring case)
/// the two are merged into that one. Returns the surviving tag.
#[tauri::command]
#[specta::specta]
pub fn rename_tag(state: State<'_, AppState>, id: String, name: String) -> AppResult<Tag> {
    state.with_world(|w| tags::rename(&w.conn, &id, &name))
}

/// Deletes a tag; its entities just lose it.
#[tauri::command]
#[specta::specta]
pub fn delete_tag(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.with_world(|w| tags::delete(&w.conn, &id))
}

/// Sets (`#rrggbb`) or clears (`null`) a tag's colour.
#[tauri::command]
#[specta::specta]
pub fn set_tag_color(
    state: State<'_, AppState>,
    id: String,
    color: Option<String>,
) -> AppResult<Tag> {
    state.with_world(|w| tags::set_color(&w.conn, &id, color.as_deref()))
}

/// Entities that carry the given tags (matched by name, ignoring case;
/// nested tags count, so `places` also finds `places/cities`), sorted by
/// type then name. `matchAll` = must have every tag, otherwise any one.
#[tauri::command]
#[specta::specta]
pub fn list_entities_by_tag(
    state: State<'_, AppState>,
    names: Vec<String>,
    match_all: bool,
) -> AppResult<Vec<EntitySummary>> {
    state.with_world(|w| tags::list_entities_by_tag(&w.conn, &names, match_all))
}
