//! Repository functions for the `calendars` table.

use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::error::AppResult;
use crate::models::calendar::{Calendar, CalendarDefinition};

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

pub fn count(conn: &Connection) -> AppResult<i64> {
    Ok(conn.query_row("SELECT count(*) FROM calendars", [], |r| r.get(0))?)
}

/// Is another calendar (not `except_id`) already called `name`?
/// `COLLATE NOCASE` makes the comparison case-insensitive.
pub fn name_taken(conn: &Connection, name: &str, except_id: Option<&str>) -> AppResult<bool> {
    let taken: bool = conn.query_row(
        "SELECT count(*) > 0 FROM calendars
         WHERE name = ?1 COLLATE NOCASE AND id <> COALESCE(?2, '')",
        params![name, except_id],
        |r| r.get(0),
    )?;
    Ok(taken)
}

pub fn insert(
    conn: &Connection,
    id: &str,
    name: &str,
    definition: &CalendarDefinition,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO calendars (id, name, definition, is_default) VALUES (?1, ?2, ?3, 0)",
        params![id, name, serde_json::to_string(definition)?],
    )?;
    Ok(())
}

pub fn update(
    conn: &Connection,
    id: &str,
    name: &str,
    definition: &CalendarDefinition,
) -> AppResult<()> {
    conn.execute(
        "UPDATE calendars SET name = ?2, definition = ?3 WHERE id = ?1",
        params![id, name, serde_json::to_string(definition)?],
    )?;
    Ok(())
}

pub fn delete(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM calendars WHERE id = ?1", params![id])?;
    Ok(())
}

/// Makes `id` the only default calendar (`id = ?1` is 1 for it, 0 for others).
pub fn set_default(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("UPDATE calendars SET is_default = (id = ?1)", params![id])?;
    Ok(())
}
