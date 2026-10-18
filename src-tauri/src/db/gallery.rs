//! Repository functions for `entity_images` (gallery images and portraits).

use std::collections::HashMap;

use rusqlite::{Connection, OptionalExtension, Row, params, params_from_iter};

use crate::db::entities::{IN_CHUNK, placeholders};
use crate::error::AppResult;
use crate::models::gallery::EntityImage;

/// Image rows joined with their asset. `path` / `thumb_path` are empty here;
/// the service fills them in from the world folder.
fn map_image(row: &Row<'_>) -> rusqlite::Result<EntityImage> {
    Ok(EntityImage {
        id: row.get("id")?,
        entity_id: row.get("entity_id")?,
        asset_id: row.get("asset_id")?,
        caption: row.get("caption")?,
        sort_order: row.get("sort_order")?,
        is_portrait: row.get::<_, i64>("is_portrait")? != 0,
        created_at: row.get("created_at")?,
        file_name: row.get("file_name")?,
        width: row.get("width")?,
        height: row.get("height")?,
        path: String::new(),
        thumb_path: None,
    })
}

const SELECT_IMAGE: &str = "SELECT i.id, i.entity_id, i.asset_id, i.caption, i.sort_order,
        i.is_portrait, i.created_at, a.file_name, a.width, a.height
     FROM entity_images i JOIN assets a ON a.id = i.asset_id";

pub fn insert(conn: &Connection, image: &EntityImage) -> AppResult<()> {
    conn.execute(
        "INSERT INTO entity_images (id, entity_id, asset_id, caption, sort_order, is_portrait, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            image.id,
            image.entity_id,
            image.asset_id,
            image.caption,
            image.sort_order,
            image.is_portrait,
            image.created_at
        ],
    )?;
    Ok(())
}

pub fn get(conn: &Connection, id: &str) -> AppResult<Option<EntityImage>> {
    Ok(conn
        .query_row(
            &format!("{SELECT_IMAGE} WHERE i.id = ?1"),
            params![id],
            map_image,
        )
        .optional()?)
}

/// The gallery of one entity in display order.
pub fn list_for_entity(conn: &Connection, entity_id: &str) -> AppResult<Vec<EntityImage>> {
    let mut stmt = conn.prepare(&format!(
        "{SELECT_IMAGE} WHERE i.entity_id = ?1 ORDER BY i.sort_order, i.created_at, i.id"
    ))?;
    let rows = stmt.query_map(params![entity_id], map_image)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn count_for_entity(conn: &Connection, entity_id: &str) -> AppResult<u32> {
    Ok(conn.query_row(
        "SELECT count(*) FROM entity_images WHERE entity_id = ?1",
        params![entity_id],
        |r| r.get(0),
    )?)
}

/// The sort order a newly appended image gets.
pub fn next_sort_order(conn: &Connection, entity_id: &str) -> AppResult<u32> {
    Ok(conn.query_row(
        "SELECT COALESCE(MAX(sort_order) + 1, 0) FROM entity_images WHERE entity_id = ?1",
        params![entity_id],
        |r| r.get(0),
    )?)
}

pub fn set_caption(conn: &Connection, id: &str, caption: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE entity_images SET caption = ?2 WHERE id = ?1",
        params![id, caption],
    )?;
    Ok(())
}

pub fn set_sort_order(conn: &Connection, id: &str, sort_order: u32) -> AppResult<()> {
    conn.execute(
        "UPDATE entity_images SET sort_order = ?2 WHERE id = ?1",
        params![id, sort_order],
    )?;
    Ok(())
}

pub fn delete(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM entity_images WHERE id = ?1", params![id])?;
    Ok(())
}

/// Makes `image_id` (or nothing, with `None`) the portrait of the entity.
/// The partial unique index allows only one portrait per entity, so the old
/// one is cleared first.
pub fn set_portrait(conn: &Connection, entity_id: &str, image_id: Option<&str>) -> AppResult<()> {
    conn.execute(
        "UPDATE entity_images SET is_portrait = 0 WHERE entity_id = ?1 AND is_portrait = 1",
        params![entity_id],
    )?;
    if let Some(id) = image_id {
        conn.execute(
            "UPDATE entity_images SET is_portrait = 1 WHERE id = ?1",
            params![id],
        )?;
    }
    Ok(())
}

/// How many places use the asset: gallery images and map backgrounds.
pub fn asset_use_count(conn: &Connection, asset_id: &str) -> AppResult<u32> {
    Ok(conn.query_row(
        "SELECT (SELECT count(*) FROM entity_images WHERE asset_id = ?1)
              + (SELECT count(*) FROM maps WHERE background_asset_id = ?1)",
        params![asset_id],
        |r| r.get(0),
    )?)
}

pub fn delete_asset_row(conn: &Connection, asset_id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM assets WHERE id = ?1", params![asset_id])?;
    Ok(())
}

/// Asset ids of every image of an entity (used before deleting the entity).
pub fn asset_ids_for_entity(conn: &Connection, entity_id: &str) -> AppResult<Vec<String>> {
    let mut stmt =
        conn.prepare("SELECT DISTINCT asset_id FROM entity_images WHERE entity_id = ?1")?;
    let rows = stmt.query_map(params![entity_id], |r| r.get(0))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// A portrait: entity id -> (asset id, mime type).
pub type PortraitRows = HashMap<String, (String, String)>;

/// Portraits of the given entities (entities without one are absent).
pub fn portraits_for(conn: &Connection, ids: &[String]) -> AppResult<PortraitRows> {
    let mut out = PortraitRows::new();
    for chunk in ids.chunks(IN_CHUNK) {
        let sql = format!(
            "SELECT i.entity_id, a.id, a.mime FROM entity_images i JOIN assets a ON a.id = i.asset_id
             WHERE i.is_portrait = 1 AND i.entity_id IN ({})",
            placeholders(chunk.len())
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(chunk.iter()), |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        for row in rows {
            let (entity, asset, mime) = row?;
            out.insert(entity, (asset, mime));
        }
    }
    Ok(out)
}

/// Every portrait in the world.
pub fn all_portraits(conn: &Connection) -> AppResult<PortraitRows> {
    let mut stmt = conn.prepare(
        "SELECT i.entity_id, a.id, a.mime FROM entity_images i JOIN assets a ON a.id = i.asset_id
         WHERE i.is_portrait = 1",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
        ))
    })?;
    let mut out = PortraitRows::new();
    for row in rows {
        let (entity, asset, mime) = row?;
        out.insert(entity, (asset, mime));
    }
    Ok(out)
}
