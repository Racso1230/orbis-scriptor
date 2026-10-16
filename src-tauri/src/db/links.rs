//! Repository functions for the `links` table.

use chrono::{SecondsFormat, Utc};
use rusqlite::{Connection, params};
use uuid::Uuid;

use crate::error::AppResult;

/// A backlink row before the service turns the text into a snippet.
pub struct BacklinkRow {
    pub source_id: String,
    pub source_type: String,
    pub source_name: String,
    pub plain_text: String,
}

/// Replaces every link of `kind` from `source_id` with the given targets.
/// Callers wrap this in a transaction together with the document save.
pub fn replace_from_source(
    conn: &Connection,
    source_id: &str,
    kind: &str,
    target_ids: &[String],
) -> AppResult<()> {
    conn.execute(
        "DELETE FROM links WHERE source_id = ?1 AND kind = ?2",
        params![source_id, kind],
    )?;
    let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    // Only insert targets that still exist: a mention of a deleted entity is
    // simply dropped. OR IGNORE then covers duplicates (UNIQUE constraint).
    let mut stmt = conn.prepare(
        "INSERT OR IGNORE INTO links (id, source_id, target_id, kind, label, created_at)
         SELECT ?1, ?2, ?3, ?4, NULL, ?5
         WHERE EXISTS (SELECT 1 FROM entities WHERE id = ?3)",
    )?;
    for target in target_ids {
        stmt.execute(params![
            Uuid::now_v7().to_string(),
            source_id,
            target,
            kind,
            now
        ])?;
    }
    Ok(())
}

/// Entities that link to `target_id` with `kind`, with their lore text.
pub fn sources_for_target(
    conn: &Connection,
    target_id: &str,
    kind: &str,
) -> AppResult<Vec<BacklinkRow>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.type, e.name, COALESCE(d.plain_text, '') AS plain_text
         FROM links l
         JOIN entities e ON e.id = l.source_id
         LEFT JOIN documents d ON d.entity_id = e.id
         WHERE l.target_id = ?1 AND l.kind = ?2
         ORDER BY e.name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map(params![target_id, kind], |row| {
        Ok(BacklinkRow {
            source_id: row.get(0)?,
            source_type: row.get(1)?,
            source_name: row.get(2)?,
            plain_text: row.get(3)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Target ids linked from `source_id` with `kind` (used by tests and sync).
pub fn targets_for_source(
    conn: &Connection,
    source_id: &str,
    kind: &str,
) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT target_id FROM links WHERE source_id = ?1 AND kind = ?2 ORDER BY target_id",
    )?;
    let rows = stmt.query_map(params![source_id, kind], |row| row.get(0))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}
