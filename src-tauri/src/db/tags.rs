//! Repository functions for `tags` and `entity_tags`.

use std::collections::HashMap;

use rusqlite::{Connection, OptionalExtension, Row, params, params_from_iter};

use crate::db::entities::{IN_CHUNK, placeholders};
use crate::error::AppResult;
use crate::models::tag::Tag;

fn map_tag(row: &Row<'_>) -> rusqlite::Result<Tag> {
    Ok(Tag {
        id: row.get("id")?,
        name: row.get("name")?,
        color: row.get("color")?,
        count: row.get("count")?,
    })
}

const SELECT_TAG: &str = "SELECT t.id, t.name, t.color,
        (SELECT count(*) FROM entity_tags et WHERE et.tag_id = t.id) AS count
     FROM tags t";

/// Every tag with its usage count, sorted by name (ignoring case).
pub fn list(conn: &Connection) -> AppResult<Vec<Tag>> {
    let mut stmt = conn.prepare(&format!("{SELECT_TAG} ORDER BY t.name COLLATE NOCASE"))?;
    let rows = stmt.query_map([], map_tag)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn get(conn: &Connection, id: &str) -> AppResult<Option<Tag>> {
    Ok(conn
        .query_row(
            &format!("{SELECT_TAG} WHERE t.id = ?1"),
            params![id],
            map_tag,
        )
        .optional()?)
}

pub fn insert(conn: &Connection, id: &str, name: &str, created_at: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO tags (id, name, color, created_at) VALUES (?1, ?2, NULL, ?3)",
        params![id, name, created_at],
    )?;
    Ok(())
}

pub fn rename(conn: &Connection, id: &str, name: &str) -> AppResult<()> {
    conn.execute("UPDATE tags SET name = ?2 WHERE id = ?1", params![id, name])?;
    Ok(())
}

pub fn set_color(conn: &Connection, id: &str, color: Option<&str>) -> AppResult<()> {
    conn.execute(
        "UPDATE tags SET color = ?2 WHERE id = ?1",
        params![id, color],
    )?;
    Ok(())
}

/// Deletes the tag; its `entity_tags` rows go with it (ON DELETE CASCADE).
pub fn delete(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM tags WHERE id = ?1", params![id])?;
    Ok(())
}

/// Moves every entity of tag `from` to tag `into`, then deletes `from`.
/// `OR IGNORE` skips entities that already carry `into`.
pub fn merge(conn: &Connection, from: &str, into: &str) -> AppResult<()> {
    conn.execute(
        "INSERT OR IGNORE INTO entity_tags (entity_id, tag_id)
         SELECT entity_id, ?2 FROM entity_tags WHERE tag_id = ?1",
        params![from, into],
    )?;
    delete(conn, from)
}

/// Ids of the entities that carry the tag.
pub fn entity_ids_for_tag(conn: &Connection, tag_id: &str) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare("SELECT entity_id FROM entity_tags WHERE tag_id = ?1")?;
    let rows = stmt.query_map(params![tag_id], |r| r.get(0))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Ids of the entities carrying ANY of the given tags.
pub fn entity_ids_for_tags(conn: &Connection, tag_ids: &[String]) -> AppResult<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    for chunk in tag_ids.chunks(IN_CHUNK) {
        let sql = format!(
            "SELECT DISTINCT entity_id FROM entity_tags WHERE tag_id IN ({})",
            placeholders(chunk.len())
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(chunk.iter()), |r| r.get::<_, String>(0))?;
        for row in rows {
            let id = row?;
            if !out.contains(&id) {
                out.push(id);
            }
        }
    }
    Ok(out)
}

/// Ids of the tags on one entity.
pub fn tag_ids_for_entity(conn: &Connection, entity_id: &str) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare("SELECT tag_id FROM entity_tags WHERE entity_id = ?1")?;
    let rows = stmt.query_map(params![entity_id], |r| r.get(0))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Replaces the tag set of one entity with exactly `tag_ids`.
pub fn replace_for_entity(conn: &Connection, entity_id: &str, tag_ids: &[String]) -> AppResult<()> {
    conn.execute(
        "DELETE FROM entity_tags WHERE entity_id = ?1",
        params![entity_id],
    )?;
    let mut stmt =
        conn.prepare("INSERT OR IGNORE INTO entity_tags (entity_id, tag_id) VALUES (?1, ?2)")?;
    for tag in tag_ids {
        stmt.execute(params![entity_id, tag])?;
    }
    Ok(())
}

/// Tag names of one entity, sorted ignoring case.
pub fn names_for_entity(conn: &Connection, entity_id: &str) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT t.name FROM entity_tags et JOIN tags t ON t.id = et.tag_id
         WHERE et.entity_id = ?1 ORDER BY t.name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map(params![entity_id], |r| r.get(0))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Tag names of EVERY tagged entity, by entity id (one query). Entities
/// without tags are absent from the map.
pub fn names_for_all(conn: &Connection) -> AppResult<HashMap<String, Vec<String>>> {
    let mut stmt = conn.prepare(
        "SELECT et.entity_id, t.name FROM entity_tags et JOIN tags t ON t.id = et.tag_id
         ORDER BY t.name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    for row in rows {
        let (entity_id, name) = row?;
        map.entry(entity_id).or_default().push(name);
    }
    Ok(map)
}

/// Like [`names_for_all`] but only for the given entities.
pub fn names_for_ids(conn: &Connection, ids: &[String]) -> AppResult<HashMap<String, Vec<String>>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    for chunk in ids.chunks(IN_CHUNK) {
        let sql = format!(
            "SELECT et.entity_id, t.name FROM entity_tags et JOIN tags t ON t.id = et.tag_id
             WHERE et.entity_id IN ({}) ORDER BY t.name COLLATE NOCASE",
            placeholders(chunk.len())
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(chunk.iter()), |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (entity_id, name) = row?;
            map.entry(entity_id).or_default().push(name);
        }
    }
    Ok(map)
}
