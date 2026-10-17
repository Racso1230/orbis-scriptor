//! Repository functions for the `calendars` table.

use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::error::AppResult;
use crate::models::calendar::Calendar;

/// The raw columns of one row: id, name, JSON definition, is_default.
type RawCalendar = (String, String, String, bool);

/// Reads the raw columns; the JSON `definition` is parsed afterwards because
/// a row-mapping closure can only return `rusqlite::Result`.
fn read_row(row: &Row<'_>) -> rusqlite::Result<RawCalendar> {
    Ok((
        row.get("id")?,
        row.get("name")?,
        row.get("definition")?,
        row.get("is_default")?,
    ))
}

fn to_calendar((id, name, definition, is_default): RawCalendar) -> AppResult<Calendar> {
    Ok(Calendar {
        id,
        name,
        // `?` converts the serde_json error into AppError (see error.rs).
        definition: serde_json::from_str(&definition)?,
        is_default,
    })
}

pub fn get(conn: &Connection, id: &str) -> AppResult<Option<Calendar>> {
    let raw = conn
        .query_row(
            "SELECT id, name, definition, is_default FROM calendars WHERE id = ?1",
            params![id],
            read_row,
        )
        .optional()?;
    raw.map(to_calendar).transpose()
}

/// The calendar flagged as default (or, defensively, the oldest one).
pub fn get_default(conn: &Connection) -> AppResult<Option<Calendar>> {
    let raw = conn
        .query_row(
            "SELECT id, name, definition, is_default FROM calendars
             ORDER BY is_default DESC, rowid LIMIT 1",
            [],
            read_row,
        )
        .optional()?;
    raw.map(to_calendar).transpose()
}

pub fn list(conn: &Connection) -> AppResult<Vec<Calendar>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, definition, is_default FROM calendars
         ORDER BY is_default DESC, name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([], read_row)?;
    rows.map(|r| to_calendar(r?)).collect()
}
