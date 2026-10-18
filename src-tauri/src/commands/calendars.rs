use tauri::State;

use crate::error::AppResult;
use crate::models::calendar::{
    Calendar, CalendarDate, CalendarDefinition, CalendarPreview, DateStyle, JsInt,
};
use crate::services::{calendar, calendar_manager};
use crate::state::AppState;

/// The world's default calendar.
#[tauri::command]
#[specta::specta]
pub fn get_default_calendar(state: State<'_, AppState>) -> AppResult<Calendar> {
    state.with_world(|w| calendar::get_default(&w.conn))
}

#[tauri::command]
#[specta::specta]
pub fn list_calendars(state: State<'_, AppState>) -> AppResult<Vec<Calendar>> {
    state.with_world(|w| calendar::list(&w.conn))
}

/// Formats one day number. `calendar_id` null = the default calendar.
#[tauri::command]
#[specta::specta]
pub fn format_day(
    state: State<'_, AppState>,
    calendar_id: Option<String>,
    day: JsInt,
    style: DateStyle,
) -> AppResult<String> {
    state.with_world(|w| {
        let cal = calendar::resolve(&w.conn, calendar_id.as_deref())?;
        calendar::format(&cal.definition, day.0, style)
    })
}

/// Formats many day numbers in one call (for timeline tick labels).
#[tauri::command]
#[specta::specta]
pub fn format_days(
    state: State<'_, AppState>,
    calendar_id: Option<String>,
    days: Vec<JsInt>,
    style: DateStyle,
) -> AppResult<Vec<String>> {
    state.with_world(|w| {
        let cal = calendar::resolve(&w.conn, calendar_id.as_deref())?;
        // `collect` into `Result<Vec<_>, _>` stops at the first error.
        days.iter()
            .map(|d| calendar::format(&cal.definition, d.0, style))
            .collect()
    })
}

/// Splits a day number into era / year / month / day (and weekday).
#[tauri::command]
#[specta::specta]
pub fn date_parts(
    state: State<'_, AppState>,
    calendar_id: Option<String>,
    day: JsInt,
) -> AppResult<CalendarDate> {
    state.with_world(|w| {
        let cal = calendar::resolve(&w.conn, calendar_id.as_deref())?;
        calendar::to_date(&cal.definition, day.0)
    })
}

/// The inverse of `date_parts`: month and day are 1-based.
#[tauri::command]
#[specta::specta]
pub fn day_from_parts(
    state: State<'_, AppState>,
    calendar_id: Option<String>,
    era_index: u32,
    year: JsInt,
    month: u32,
    day: u32,
) -> AppResult<JsInt> {
    state.with_world(|w| {
        let cal = calendar::resolve(&w.conn, calendar_id.as_deref())?;
        calendar::from_date(&cal.definition, era_index, year.0, month, day).map(JsInt)
    })
}

#[tauri::command]
#[specta::specta]
pub fn get_calendar(state: State<'_, AppState>, id: String) -> AppResult<Calendar> {
    state.with_world(|w| calendar_manager::get(&w.conn, &id))
}

/// Creates a calendar. Fails with a validation error naming the bad fields.
#[tauri::command]
#[specta::specta]
pub fn create_calendar(
    state: State<'_, AppState>,
    name: String,
    definition: CalendarDefinition,
) -> AppResult<Calendar> {
    state.with_world(|w| calendar_manager::create(&w.conn, &name, definition))
}

/// Renames and/or redefines a calendar (null = keep). Events never move:
/// changing month lengths, leap rules or the epoch offset only changes how
/// the existing absolute days are displayed.
#[tauri::command]
#[specta::specta]
pub fn update_calendar(
    state: State<'_, AppState>,
    id: String,
    name: Option<String>,
    definition: Option<CalendarDefinition>,
) -> AppResult<Calendar> {
    state.with_world(|w| calendar_manager::update(&w.conn, &id, name.as_deref(), definition))
}

/// Deletes a calendar (never the default one). Its events move to the
/// default calendar.
#[tauri::command]
#[specta::specta]
pub fn delete_calendar(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.with_world(|w| calendar_manager::delete(&w.conn, &id))
}

#[tauri::command]
#[specta::specta]
pub fn set_default_calendar(state: State<'_, AppState>, id: String) -> AppResult<Calendar> {
    state.with_world(|w| calendar_manager::set_default(&w.conn, &id))
}

#[tauri::command]
#[specta::specta]
pub fn duplicate_calendar(
    state: State<'_, AppState>,
    id: String,
    new_name: String,
) -> AppResult<Calendar> {
    state.with_world(|w| calendar_manager::duplicate(&w.conn, &id, &new_name))
}

/// Formats `days` with an UNSAVED definition (live preview in the editor).
/// Problems come back in `issues` (per field) instead of as an error.
#[tauri::command]
#[specta::specta]
pub fn preview_calendar(
    definition: CalendarDefinition,
    days: Vec<JsInt>,
    style: DateStyle,
) -> CalendarPreview {
    let days: Vec<i64> = days.iter().map(|d| d.0).collect();
    calendar_manager::preview(&definition, &days, style)
}

/// Number of days in a month (1-based) of an era year, for date editors.
/// `calendar_id` null = the default calendar.
#[tauri::command]
#[specta::specta]
pub fn days_in_month(
    state: State<'_, AppState>,
    calendar_id: Option<String>,
    era_index: u32,
    year: JsInt,
    month: u32,
) -> AppResult<u32> {
    state.with_world(|w| {
        calendar_manager::days_in_month(&w.conn, calendar_id.as_deref(), era_index, year.0, month)
    })
}

/// The calendar the world is displayed in (the default one until chosen).
#[tauri::command]
#[specta::specta]
pub fn get_display_calendar(state: State<'_, AppState>) -> AppResult<Calendar> {
    state.with_world(|w| calendar_manager::get_display(&w.conn))
}

/// Chooses the display calendar; null = follow the default calendar.
#[tauri::command]
#[specta::specta]
pub fn set_display_calendar(
    state: State<'_, AppState>,
    calendar_id: Option<String>,
) -> AppResult<Calendar> {
    state.with_world(|w| calendar_manager::set_display(&w.conn, calendar_id.as_deref()))
}
