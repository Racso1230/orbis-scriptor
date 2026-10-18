//! Commands for user-defined entity types and field templates. The list of
//! types itself comes from `entities::entity_types`.

use tauri::State;

use crate::error::AppResult;
use crate::models::entity_type::{EntityTypeDef, FieldInput};
use crate::services::entity_types;
use crate::state::AppState;

/// The icon names a custom type may use (lucide names).
#[tauri::command]
#[specta::specta]
pub fn list_type_icons() -> Vec<String> {
    entity_types::type_icons()
}

/// Creates a custom type. `color` is `#RRGGBB` (empty = default). Field keys
/// are generated from the labels.
#[tauri::command]
#[specta::specta]
pub fn create_entity_type(
    state: State<'_, AppState>,
    label: String,
    plural: String,
    icon: String,
    color: String,
    fields: Vec<FieldInput>,
) -> AppResult<EntityTypeDef> {
    state.with_world(|w| entity_types::create(&w.conn, &label, &plural, &icon, &color, fields))
}

/// Edits a custom type; `null` arguments stay unchanged. `fields`, when
/// given, replaces the whole field list (send existing fields with their keys).
#[tauri::command]
#[specta::specta]
pub fn update_entity_type(
    state: State<'_, AppState>,
    key: String,
    label: Option<String>,
    plural: Option<String>,
    icon: Option<String>,
    color: Option<String>,
    fields: Option<Vec<FieldInput>>,
) -> AppResult<EntityTypeDef> {
    state.with_world(|w| {
        entity_types::update(
            &w.conn,
            &key,
            label.as_deref(),
            plural.as_deref(),
            icon.as_deref(),
            color.as_deref(),
            fields,
        )
    })
}

/// Deletes a custom type. When entities of the type exist, `reassign_to`
/// must name another type; they are converted in one transaction.
#[tauri::command]
#[specta::specta]
pub fn delete_entity_type(
    state: State<'_, AppState>,
    key: String,
    reassign_to: Option<String>,
) -> AppResult<()> {
    state.with_world(|w| entity_types::delete(&w.conn, &key, reassign_to.as_deref()))
}

/// Sets the extra user fields of a built-in type (the built-in fields stay
/// fixed). For a custom type this replaces its field list.
#[tauri::command]
#[specta::specta]
pub fn set_type_template_fields(
    state: State<'_, AppState>,
    type_key: String,
    fields: Vec<FieldInput>,
) -> AppResult<EntityTypeDef> {
    state.with_world(|w| entity_types::set_template_fields(&w.conn, &type_key, fields))
}

/// Sets the display order of the custom types (all keys, once each) and
/// returns the full merged type list.
#[tauri::command]
#[specta::specta]
pub fn reorder_entity_types(
    state: State<'_, AppState>,
    keys: Vec<String>,
) -> AppResult<Vec<EntityTypeDef>> {
    state.with_world(|w| entity_types::reorder(&w.conn, &keys))
}
