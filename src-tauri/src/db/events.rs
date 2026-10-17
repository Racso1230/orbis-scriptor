//! Repository functions for the `events` table (joined with `entities` for
//! the event's name).

use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::error::{AppError, AppResult};
use crate::models::event::EventSummary;

fn map_event(row: &Row<'_>) -> rusqlite::Result<EventSummary> {
    Ok(EventSummary {
        entity_id: row.get("entity_id")?,
        name: row.get("name")?,
        start_day: row.get("start_day")?,
        end_day: row.get("end_day")?,
        importance: row.get("importance")?,
        calendar_id: row.get("calendar_id")?,
    })
}

const SELECT_EVENT: &str = "SELECT ev.entity_id, e.name, ev.start_day, ev.end_day,
        ev.importance, ev.calendar_id
     FROM events ev JOIN entities e ON e.id = ev.entity_id";

pub fn insert(
    conn: &Connection,
    entity_id: &str,
    calendar_id: &str,
    start_day: i64,
    end_day: Option<i64>,
    importance: i32,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO events (entity_id, calendar_id, start_day, end_day, importance)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![entity_id, calendar_id, start_day, end_day, importance],
    )?;
    Ok(())
}

pub fn get(conn: &Connection, entity_id: &str) -> AppResult<Option<EventSummary>> {
    let event = conn
        .query_row(
            &format!("{SELECT_EVENT} WHERE ev.entity_id = ?1"),
            params![entity_id],
            map_event,
        )
        .optional()?;
    Ok(event)
}

/// Events that overlap the closed window `[from_day, to_day]`, oldest first.
/// An event overlaps when it starts on/before the window's end and finishes
/// (its end day, or its start day for single-day events) on/after the
/// window's start. `idx_events_start` serves the first condition.
pub fn list_range(conn: &Connection, from_day: i64, to_day: i64) -> AppResult<Vec<EventSummary>> {
    let mut stmt = conn.prepare(&format!(
        "{SELECT_EVENT}
         WHERE ev.start_day <= ?2 AND COALESCE(ev.end_day, ev.start_day) >= ?1
         ORDER BY ev.start_day, COALESCE(ev.end_day, ev.start_day), e.name COLLATE NOCASE"
    ))?;
    let rows = stmt.query_map(params![from_day, to_day], map_event)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Events the entity is involved in (`involved_in` links), oldest first.
pub fn list_for_entity(conn: &Connection, entity_id: &str) -> AppResult<Vec<EventSummary>> {
    let mut stmt = conn.prepare(&format!(
        "{SELECT_EVENT}
         JOIN links l ON l.target_id = ev.entity_id
         WHERE l.source_id = ?1 AND l.kind = ?2
         ORDER BY ev.start_day, e.name COLLATE NOCASE"
    ))?;
    let rows = stmt.query_map(
        params![entity_id, crate::models::link::KIND_INVOLVED_IN],
        map_event,
    )?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn update_dates(
    conn: &Connection,
    entity_id: &str,
    start_day: i64,
    end_day: Option<i64>,
    importance: Option<i32>,
) -> AppResult<()> {
    // COALESCE(?4, importance) keeps the old value when `importance` is NULL.
    let changed = conn.execute(
        "UPDATE events SET start_day = ?2, end_day = ?3, importance = COALESCE(?4, importance)
         WHERE entity_id = ?1",
        params![entity_id, start_day, end_day, importance],
    )?;
    if changed == 0 {
        return Err(AppError::NotFound(format!(
            "Event {entity_id} does not exist"
        )));
    }
    Ok(())
}
