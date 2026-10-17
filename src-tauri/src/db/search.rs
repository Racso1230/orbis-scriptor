//! SQL for the `search_index` FTS5 table (created in migration 005).
//! One row per entity: `name`, `summary` and `body` (lore plain text).

use rusqlite::{Connection, params};

use crate::error::AppResult;
use crate::models::search::{HIGHLIGHT_END, HIGHLIGHT_START, SearchResult};

/// Replaces the index row of one entity. FTS5 tables have no UNIQUE
/// constraints, so "upsert" is delete-then-insert.
pub fn replace(
    conn: &Connection,
    entity_id: &str,
    name: &str,
    summary: &str,
    body: &str,
) -> AppResult<()> {
    delete(conn, entity_id)?;
    conn.execute(
        "INSERT INTO search_index (entity_id, name, summary, body) VALUES (?1, ?2, ?3, ?4)",
        params![entity_id, name, summary, body],
    )?;
    Ok(())
}

pub fn delete(conn: &Connection, entity_id: &str) -> AppResult<()> {
    conn.execute(
        "DELETE FROM search_index WHERE entity_id = ?1",
        params![entity_id],
    )?;
    Ok(())
}

/// Empties the index and refills it from `entities` + `documents`.
pub fn rebuild_all(conn: &Connection) -> AppResult<()> {
    conn.execute("DELETE FROM search_index", [])?;
    conn.execute(
        "INSERT INTO search_index (entity_id, name, summary, body)
         SELECT e.id, e.name, e.summary, COALESCE(d.plain_text, '')
         FROM entities e LEFT JOIN documents d ON d.entity_id = e.id",
        [],
    )?;
    Ok(())
}

/// Runs an already-sanitised FTS5 `MATCH` expression.
///
/// - `bm25(search_index, 0, 10, 4, 1)` gives per-column weights in column
///   order (entity_id, name, summary, body): a name hit counts 10x a lore
///   hit. bm25 returns *lower = better*, so we sort ascending.
/// - `snippet(..., -1, ...)` lets SQLite pick the best column to excerpt.
pub fn query(conn: &Connection, match_expr: &str, limit: u32) -> AppResult<Vec<SearchResult>> {
    let mut stmt = conn.prepare(
        "SELECT search_index.entity_id, e.type, e.name,
                snippet(search_index, -1, ?2, ?3, '…', 12)
         FROM search_index
         JOIN entities e ON e.id = search_index.entity_id
         WHERE search_index MATCH ?1
         ORDER BY bm25(search_index, 0.0, 10.0, 4.0, 1.0)
         LIMIT ?4",
    )?;
    let rows = stmt.query_map(
        params![
            match_expr,
            HIGHLIGHT_START.to_string(),
            HIGHLIGHT_END.to_string(),
            limit
        ],
        |row| {
            Ok(SearchResult {
                entity_id: row.get(0)?,
                entity_type: row.get(1)?,
                name: row.get(2)?,
                snippet: row.get(3)?,
            })
        },
    )?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}
