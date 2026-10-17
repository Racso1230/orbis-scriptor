use tauri::State;

use crate::error::{AppError, AppResult};
use crate::models::calendar::JsInt;
use crate::models::entity::EntitySummary;
use crate::models::event::EventSummary;
use crate::services::{dev_seed, events};
use crate::state::AppState;

/// Events overlapping the closed day window `[from_day, to_day]`, oldest first.
#[tauri::command]
#[specta::specta]
pub fn list_events(
    state: State<'_, AppState>,
    from_day: JsInt,
    to_day: JsInt,
) -> AppResult<Vec<EventSummary>> {
    state.with_world(|w| events::list(&w.conn, from_day.0, to_day.0))
}

#[tauri::command]
#[specta::specta]
pub fn get_event(state: State<'_, AppState>, entity_id: String) -> AppResult<EventSummary> {
    state.with_world(|w| events::get(&w.conn, &entity_id))
}

/// Creates the event entity and its dates in one transaction.
#[tauri::command]
#[specta::specta]
pub fn create_event(
    state: State<'_, AppState>,
    name: String,
    start_day: JsInt,
    end_day: Option<JsInt>,
) -> AppResult<EventSummary> {
    state.with_world(|w| events::create(&w.conn, &name, start_day.0, end_day.map(|d| d.0)))
}

/// Changes the dates (end must not be before start). `importance` (1-5) is
/// left unchanged when null.
#[tauri::command]
#[specta::specta]
pub fn update_event_dates(
    state: State<'_, AppState>,
    entity_id: String,
    start_day: JsInt,
    end_day: Option<JsInt>,
    importance: Option<i32>,
) -> AppResult<EventSummary> {
    state.with_world(|w| {
        events::update_dates(
            &w.conn,
            &entity_id,
            start_day.0,
            end_day.map(|d| d.0),
            importance,
        )
    })
}

/// Entities involved in the event (`involved_in` links).
#[tauri::command]
#[specta::specta]
pub fn list_involved(
    state: State<'_, AppState>,
    event_id: String,
) -> AppResult<Vec<EntitySummary>> {
    state.with_world(|w| events::list_involved(&w.conn, &event_id))
}

#[tauri::command]
#[specta::specta]
pub fn add_involved(
    state: State<'_, AppState>,
    event_id: String,
    entity_id: String,
) -> AppResult<()> {
    state.with_world(|w| events::add_involved(&w.conn, &event_id, &entity_id))
}

#[tauri::command]
#[specta::specta]
pub fn remove_involved(
    state: State<'_, AppState>,
    event_id: String,
    entity_id: String,
) -> AppResult<()> {
    state.with_world(|w| events::remove_involved(&w.conn, &event_id, &entity_id))
}

/// Events the entity is involved in, oldest first.
#[tauri::command]
#[specta::specta]
pub fn list_events_for_entity(
    state: State<'_, AppState>,
    entity_id: String,
) -> AppResult<Vec<EventSummary>> {
    state.with_world(|w| events::list_for_entity(&w.conn, &entity_id))
}

/// Development only: creates `count` events spread over a few thousand years,
/// some of them ranges, linked to random characters. Refuses to run in release.
#[tauri::command]
#[specta::specta]
pub fn dev_seed_events(state: State<'_, AppState>, count: u32) -> AppResult<u32> {
    if !cfg!(debug_assertions) {
        return Err(AppError::Validation(
            "dev_seed_events is only available in debug builds".into(),
        ));
    }
    state.with_world(|w| dev_seed::seed_events(&w.conn, count))
}
