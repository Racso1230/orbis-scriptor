//! Repository functions for the `assets` table.

use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::error::AppResult;
use crate::models::map::Asset;

/// Maps a row to an `Asset`. `path` and `thumb_path` are not stored in the
/// database, so they start empty here and the service fills them in.
fn map_asset(row: &Row<'_>) -> rusqlite::Result<Asset> {
    Ok(Asset {
        id: row.get("id")?,
        file_name: row.get("file_name")?,
        mime: row.get("mime")?,
        width: row.get("width")?,
        height: row.get("height")?,
        created_at: row.get("created_at")?,
        path: String::new(),
        thumb_path: None,
    })
}

pub fn insert(conn: &Connection, asset: &Asset) -> AppResult<()> {
    conn.execute(
        "INSERT INTO assets (id, file_name, mime, width, height, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            asset.id,
            asset.file_name,
            asset.mime,
            asset.width,
            asset.height,
            asset.created_at
        ],
    )?;
    Ok(())
}

pub fn get(conn: &Connection, id: &str) -> AppResult<Option<Asset>> {
    Ok(conn
        .query_row("SELECT * FROM assets WHERE id = ?1", params![id], map_asset)
        .optional()?)
}
