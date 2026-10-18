use tauri::State;

use crate::error::{AppError, AppResult};
use crate::models::relationship::{
    EntityGraph, FamilyTree, GraphFilter, RelationshipKindDef, RelationshipRecord, RelationshipView,
};
use crate::services::{dev_seed, graph, relationships};
use crate::state::AppState;

/// The built-in relationship catalog (labels, symmetry, categories,
/// type constraints). The frontend renders pickers and filters from this.
#[tauri::command]
#[specta::specta]
pub fn list_relationship_kinds() -> Vec<RelationshipKindDef> {
    relationships::list_kinds()
}

/// Relationships of one entity, outgoing and incoming, phrased from its side.
#[tauri::command]
#[specta::specta]
pub fn list_relationships(
    state: State<'_, AppState>,
    entity_id: String,
) -> AppResult<Vec<RelationshipView>> {
    state.with_world(|w| relationships::list_for_entity(&w.conn, &entity_id))
}

/// Creates "source <kind> target". `label` is only used by `custom`.
#[tauri::command]
#[specta::specta]
pub fn create_relationship(
    state: State<'_, AppState>,
    source_id: String,
    target_id: String,
    kind: String,
    label: Option<String>,
) -> AppResult<RelationshipRecord> {
    state.with_world(|w| relationships::create(&w.conn, &source_id, &target_id, &kind, label))
}

/// Changes kind, label and/or direction; null fields stay as they are.
#[tauri::command]
#[specta::specta]
pub fn update_relationship(
    state: State<'_, AppState>,
    link_id: String,
    kind: Option<String>,
    label: Option<String>,
    swap_direction: Option<bool>,
) -> AppResult<RelationshipRecord> {
    state.with_world(|w| relationships::update(&w.conn, &link_id, kind, label, swap_direction))
}

#[tauri::command]
#[specta::specta]
pub fn delete_relationship(state: State<'_, AppState>, link_id: String) -> AppResult<()> {
    state.with_world(|w| relationships::delete(&w.conn, &link_id))
}

/// The graph around `focus_id`, `depth` (1-4) links deep.
#[tauri::command]
#[specta::specta]
pub fn get_entity_graph(
    state: State<'_, AppState>,
    focus_id: String,
    depth: u32,
    filter: GraphFilter,
) -> AppResult<EntityGraph> {
    state.with_world(|w| graph::get_entity_graph(&w.conn, &focus_id, depth, &filter))
}

/// Family tree around a character; each node has a generation number
/// (negative = ancestors). Both limits default to 3.
#[tauri::command]
#[specta::specta]
pub fn get_family_tree(
    state: State<'_, AppState>,
    focus_id: String,
    generations_up: Option<u32>,
    generations_down: Option<u32>,
) -> AppResult<FamilyTree> {
    state.with_world(|w| {
        graph::get_family_tree(&w.conn, &focus_id, generations_up, generations_down)
    })
}

/// Development only: creates up to `count` random valid relationships among
/// existing entities. Refuses to run in release builds.
#[tauri::command]
#[specta::specta]
pub fn dev_seed_relationships(state: State<'_, AppState>, count: u32) -> AppResult<u32> {
    if !cfg!(debug_assertions) {
        return Err(AppError::Validation(
            "dev_seed_relationships is only available in debug builds".into(),
        ));
    }
    state.with_world(|w| dev_seed::seed_relationships(&w.conn, count))
}
