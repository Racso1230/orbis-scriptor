use tauri::State;

use crate::error::AppResult;
use crate::models::calendar::{Calendar, CalendarDate, DateStyle, JsInt};
use crate::services::calendar;
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
