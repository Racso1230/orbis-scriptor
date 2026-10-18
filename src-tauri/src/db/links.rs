//! Repository functions for the `links` table.

use chrono::{SecondsFormat, Utc};
use rusqlite::{Connection, params};
use uuid::Uuid;

use crate::db::entities::{IN_CHUNK, placeholders};
use crate::error::AppResult;
use crate::models::entity::EntitySummary;

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

/// Adds one link; does nothing if it already exists (UNIQUE constraint).
pub fn insert_link(
    conn: &Connection,
    source_id: &str,
    target_id: &str,
    kind: &str,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    conn.execute(
        "INSERT OR IGNORE INTO links (id, source_id, target_id, kind, label, created_at)
         VALUES (?1, ?2, ?3, ?4, NULL, ?5)",
        params![Uuid::now_v7().to_string(), source_id, target_id, kind, now],
    )?;
    Ok(())
}

/// Removes one link; returns whether a row was deleted.
pub fn delete_link(
    conn: &Connection,
    source_id: &str,
    target_id: &str,
    kind: &str,
) -> AppResult<bool> {
    let changed = conn.execute(
        "DELETE FROM links WHERE source_id = ?1 AND target_id = ?2 AND kind = ?3",
        params![source_id, target_id, kind],
    )?;
    Ok(changed > 0)
}

/// The entities that link to `target_id` with `kind`, by name.
pub fn list_sources(
    conn: &Connection,
    target_id: &str,
    kind: &str,
) -> AppResult<Vec<EntitySummary>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.type, e.name, e.updated_at
         FROM links l JOIN entities e ON e.id = l.source_id
         WHERE l.target_id = ?1 AND l.kind = ?2
         ORDER BY e.type, e.name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map(params![target_id, kind], |row| {
        Ok(EntitySummary {
            id: row.get(0)?,
            entity_type: row.get(1)?,
            name: row.get(2)?,
            updated_at: row.get(3)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

// --- Relationships (Phase 9) -------------------------------------------
// These functions deal with whole `links` rows by id. Callers (the
// relationships service) are responsible for never passing the reserved
// `mention` / `involved_in` kinds.

/// One full `links` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkRow {
    pub id: String,
    pub source_id: String,
    pub target_id: String,
    pub kind: String,
    pub label: Option<String>,
}

fn map_link(row: &rusqlite::Row<'_>) -> rusqlite::Result<LinkRow> {
    Ok(LinkRow {
        id: row.get(0)?,
        source_id: row.get(1)?,
        target_id: row.get(2)?,
        kind: row.get(3)?,
        label: row.get(4)?,
    })
}

const LINK_COLUMNS: &str = "id, source_id, target_id, kind, label";

pub fn get_link(conn: &Connection, id: &str) -> AppResult<Option<LinkRow>> {
    use rusqlite::OptionalExtension;
    Ok(conn
        .query_row(
            &format!("SELECT {LINK_COLUMNS} FROM links WHERE id = ?1"),
            params![id],
            map_link,
        )
        .optional()?)
}

/// The link row with exactly this (source, target, kind), if any.
pub fn find_link(
    conn: &Connection,
    source_id: &str,
    target_id: &str,
    kind: &str,
) -> AppResult<Option<LinkRow>> {
    use rusqlite::OptionalExtension;
    Ok(conn
        .query_row(
            &format!(
                "SELECT {LINK_COLUMNS} FROM links
                 WHERE source_id = ?1 AND target_id = ?2 AND kind = ?3"
            ),
            params![source_id, target_id, kind],
            map_link,
        )
        .optional()?)
}

/// Inserts a new row; fails (UNIQUE) if it already exists.
pub fn insert_row(conn: &Connection, row: &LinkRow) -> AppResult<()> {
    let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    conn.execute(
        "INSERT INTO links (id, source_id, target_id, kind, label, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            row.id,
            row.source_id,
            row.target_id,
            row.kind,
            row.label,
            now
        ],
    )?;
    Ok(())
}

/// Overwrites direction, kind and label of an existing row.
pub fn update_row(conn: &Connection, row: &LinkRow) -> AppResult<()> {
    conn.execute(
        "UPDATE links SET source_id = ?2, target_id = ?3, kind = ?4, label = ?5 WHERE id = ?1",
        params![row.id, row.source_id, row.target_id, row.kind, row.label],
    )?;
    Ok(())
}

pub fn delete_by_id(conn: &Connection, id: &str) -> AppResult<bool> {
    let changed = conn.execute("DELETE FROM links WHERE id = ?1", params![id])?;
    Ok(changed > 0)
}

/// A link of a given entity joined with both endpoints' names and types.
pub struct LinkWithEnds {
    pub link: LinkRow,
    pub source_name: String,
    pub source_type: String,
    pub target_name: String,
    pub target_type: String,
}

/// Every link touching `entity_id` (either end) whose kind is in `kinds`.
pub fn list_for_entity(
    conn: &Connection,
    entity_id: &str,
    kinds: &[String],
) -> AppResult<Vec<LinkWithEnds>> {
    let sql = format!(
        "SELECT l.id, l.source_id, l.target_id, l.kind, l.label,
                s.name, s.type, t.name, t.type
         FROM links l
         JOIN entities s ON s.id = l.source_id
         JOIN entities t ON t.id = l.target_id
         WHERE (l.source_id = ? OR l.target_id = ?) AND l.kind IN ({})
         ORDER BY l.created_at, l.id",
        placeholders(kinds.len())
    );
    let mut stmt = conn.prepare(&sql)?;
    // Parameters: the entity id twice, then every kind.
    let mut args: Vec<&str> = vec![entity_id, entity_id];
    args.extend(kinds.iter().map(String::as_str));
    let rows = stmt.query_map(rusqlite::params_from_iter(args), |row| {
        Ok(LinkWithEnds {
            link: map_link(row)?,
            source_name: row.get(5)?,
            source_type: row.get(6)?,
            target_name: row.get(7)?,
            target_type: row.get(8)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// All links of the given kinds that touch any of `ids` (either end). One
/// query per chunk of ids, so a BFS level costs one round trip in practice.
/// Rows can repeat when a link joins two ids of different chunks, so callers
/// de-duplicate by `LinkRow::id`.
pub fn edges_touching(
    conn: &Connection,
    ids: &[String],
    kinds: &[String],
) -> AppResult<Vec<LinkRow>> {
    let mut out = Vec::new();
    for chunk in ids.chunks(IN_CHUNK) {
        let marks = placeholders(chunk.len());
        let sql = format!(
            "SELECT {LINK_COLUMNS} FROM links
             WHERE (source_id IN ({marks}) OR target_id IN ({marks}))
               AND kind IN ({})
             ORDER BY id",
            placeholders(kinds.len())
        );
        let mut stmt = conn.prepare(&sql)?;
        // The id list is bound twice (once per IN), then the kinds.
        let args: Vec<&str> = chunk
            .iter()
            .chain(chunk.iter())
            .chain(kinds.iter())
            .map(String::as_str)
            .collect();
        let rows = stmt.query_map(rusqlite::params_from_iter(args), map_link)?;
        for row in rows {
            out.push(row?);
        }
    }
    Ok(out)
}
