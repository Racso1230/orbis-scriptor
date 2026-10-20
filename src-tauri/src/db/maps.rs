//! Repository functions for `maps`, `map_layers` and `map_objects`.
//! Geometry and style are stored as JSON text and parsed back here, so
//! callers only ever see typed values.

use rusqlite::{Connection, OptionalExtension, Row, params};
use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::models::map::{
    Asset, MapAppearance, MapBackground, MapGeometry, MapInfo, MapLayer, MapObject,
};

// ---- maps ---------------------------------------------------------------

/// Inserts a `maps` row. Image maps start at 0x0 with no style; blank maps
/// pass their size and look.
pub fn insert_map(
    conn: &Connection,
    id: &str,
    entity_id: &str,
    width: u32,
    height: u32,
    style: Option<&MapBackground>,
) -> AppResult<()> {
    // `Option<String>` becomes SQL NULL when it is `None`.
    let style_json = style.map(serde_json::to_string).transpose()?;
    conn.execute(
        "INSERT INTO maps (id, entity_id, background_asset_id, width, height, background_style)
         VALUES (?1, ?2, NULL, ?3, ?4, ?5)",
        params![id, entity_id, width, height, style_json],
    )?;
    Ok(())
}

/// Shared SELECT for `MapInfo`: the map, its entity's name, and the background
/// asset columns (prefixed `a_`) from a LEFT JOIN.
const MAP_SELECT: &str = "SELECT m.id, m.entity_id, e.name, m.width, m.height, m.background_style,
        a.id AS a_id, a.file_name AS a_file_name, a.mime AS a_mime,
        a.width AS a_width, a.height AS a_height, a.created_at AS a_created_at
     FROM maps m
     JOIN entities e ON e.id = m.entity_id
     LEFT JOIN assets a ON a.id = m.background_asset_id";

fn map_info(row: &Row<'_>) -> rusqlite::Result<MapInfo> {
    // The LEFT JOIN yields NULL asset columns when there is no background.
    let background = match row.get::<_, Option<String>>("a_id")? {
        Some(id) => Some(Asset {
            id,
            file_name: row.get("a_file_name")?,
            mime: row.get("a_mime")?,
            width: row.get("a_width")?,
            height: row.get("a_height")?,
            created_at: row.get("a_created_at")?,
            path: String::new(),
            thumb_path: None,
        }),
        None => None,
    };
    // Style JSON that cannot be parsed is treated as "no style".
    let style: Option<String> = row.get("background_style")?;
    let background_style = style.and_then(|text| serde_json::from_str(&text).ok());
    Ok(MapInfo {
        id: row.get("id")?,
        entity_id: row.get("entity_id")?,
        name: row.get("name")?,
        width: row.get("width")?,
        height: row.get("height")?,
        background,
        background_style,
    })
}

pub fn get_map_by_entity(conn: &Connection, entity_id: &str) -> AppResult<Option<MapInfo>> {
    let sql = format!("{MAP_SELECT} WHERE m.entity_id = ?1");
    Ok(conn
        .query_row(&sql, params![entity_id], map_info)
        .optional()?)
}

pub fn get_map(conn: &Connection, map_id: &str) -> AppResult<Option<MapInfo>> {
    let sql = format!("{MAP_SELECT} WHERE m.id = ?1");
    Ok(conn.query_row(&sql, params![map_id], map_info).optional()?)
}

/// Sets an image background. The blank-map style is cleared: an image map
/// always has `background_style = NULL`.
pub fn set_background(
    conn: &Connection,
    map_id: &str,
    asset_id: &str,
    width: u32,
    height: u32,
) -> AppResult<()> {
    let changed = conn.execute(
        "UPDATE maps SET background_asset_id = ?2, width = ?3, height = ?4, background_style = NULL
         WHERE id = ?1",
        params![map_id, asset_id, width, height],
    )?;
    if changed == 0 {
        return Err(AppError::NotFound(format!("Map {map_id} does not exist")));
    }
    Ok(())
}

pub fn set_background_style(
    conn: &Connection,
    map_id: &str,
    style: &MapBackground,
) -> AppResult<()> {
    conn.execute(
        "UPDATE maps SET background_style = ?2 WHERE id = ?1",
        params![map_id, serde_json::to_string(style)?],
    )?;
    Ok(())
}

pub fn set_size(conn: &Connection, map_id: &str, width: u32, height: u32) -> AppResult<()> {
    conn.execute(
        "UPDATE maps SET width = ?2, height = ?3 WHERE id = ?1",
        params![map_id, width, height],
    )?;
    Ok(())
}

// ---- layers -------------------------------------------------------------

pub fn insert_layer(conn: &Connection, layer: &MapLayer) -> AppResult<()> {
    conn.execute(
        "INSERT INTO map_layers (id, map_id, name, sort_order, visible, locked)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            layer.id,
            layer.map_id,
            layer.name,
            layer.sort_order,
            layer.visible,
            layer.locked
        ],
    )?;
    Ok(())
}

const LAYER_SELECT: &str = "SELECT l.id, l.map_id, l.name, l.sort_order, l.visible, l.locked
     FROM map_layers l";

fn map_layer(row: &Row<'_>) -> rusqlite::Result<MapLayer> {
    Ok(MapLayer {
        id: row.get("id")?,
        map_id: row.get("map_id")?,
        name: row.get("name")?,
        sort_order: row.get("sort_order")?,
        visible: row.get("visible")?,
        locked: row.get("locked")?,
    })
}

pub fn list_layers(conn: &Connection, map_id: &str) -> AppResult<Vec<MapLayer>> {
    let sql = format!("{LAYER_SELECT} WHERE l.map_id = ?1 ORDER BY l.sort_order, l.id");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![map_id], map_layer)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn get_layer(conn: &Connection, layer_id: &str) -> AppResult<Option<MapLayer>> {
    let sql = format!("{LAYER_SELECT} WHERE l.id = ?1");
    Ok(conn
        .query_row(&sql, params![layer_id], map_layer)
        .optional()?)
}

/// The layer an object lives on.
pub fn get_layer_for_object(conn: &Connection, object_id: &str) -> AppResult<Option<MapLayer>> {
    let sql = format!("{LAYER_SELECT} JOIN map_objects o ON o.layer_id = l.id WHERE o.id = ?1");
    Ok(conn
        .query_row(&sql, params![object_id], map_layer)
        .optional()?)
}

/// Highest `sort_order` of a map's layers, or `None` if it has no layers.
pub fn max_layer_order(conn: &Connection, map_id: &str) -> AppResult<Option<i32>> {
    Ok(conn.query_row(
        "SELECT MAX(sort_order) FROM map_layers WHERE map_id = ?1",
        params![map_id],
        |row| row.get(0),
    )?)
}

pub fn count_layers(conn: &Connection, map_id: &str) -> AppResult<i64> {
    Ok(conn.query_row(
        "SELECT count(*) FROM map_layers WHERE map_id = ?1",
        params![map_id],
        |row| row.get(0),
    )?)
}

/// Number of objects on all layers of a map.
pub fn count_objects(conn: &Connection, map_id: &str) -> AppResult<i64> {
    Ok(conn.query_row(
        "SELECT count(*) FROM map_objects o JOIN map_layers l ON l.id = o.layer_id
         WHERE l.map_id = ?1",
        params![map_id],
        |row| row.get(0),
    )?)
}

pub fn rename_layer(conn: &Connection, layer_id: &str, name: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE map_layers SET name = ?2 WHERE id = ?1",
        params![layer_id, name],
    )?;
    Ok(())
}

pub fn set_layer_visible(conn: &Connection, layer_id: &str, visible: bool) -> AppResult<()> {
    conn.execute(
        "UPDATE map_layers SET visible = ?2 WHERE id = ?1",
        params![layer_id, visible],
    )?;
    Ok(())
}

pub fn set_layer_locked(conn: &Connection, layer_id: &str, locked: bool) -> AppResult<()> {
    conn.execute(
        "UPDATE map_layers SET locked = ?2 WHERE id = ?1",
        params![layer_id, locked],
    )?;
    Ok(())
}

pub fn set_layer_order(conn: &Connection, layer_id: &str, sort_order: i32) -> AppResult<()> {
    conn.execute(
        "UPDATE map_layers SET sort_order = ?2 WHERE id = ?1",
        params![layer_id, sort_order],
    )?;
    Ok(())
}

/// Deletes the layer; its objects go with it (ON DELETE CASCADE).
pub fn delete_layer(conn: &Connection, layer_id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM map_layers WHERE id = ?1", params![layer_id])?;
    Ok(())
}

pub fn layer_exists(conn: &Connection, layer_id: &str) -> AppResult<bool> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM map_layers WHERE id = ?1",
            params![layer_id],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

/// The default (first) layer of a map.
pub fn first_layer_id(conn: &Connection, map_id: &str) -> AppResult<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT id FROM map_layers WHERE map_id = ?1 ORDER BY sort_order, id LIMIT 1",
            params![map_id],
            |row| row.get(0),
        )
        .optional()?)
}

// ---- objects ------------------------------------------------------------

/// SELECT with the linked entity's type and name joined in.
const OBJECT_SELECT: &str = "SELECT o.id, o.layer_id, o.geometry, o.style, o.entity_id,
        o.created_at, o.updated_at, e.type AS entity_type, e.name AS entity_name
     FROM map_objects o
     LEFT JOIN entities e ON e.id = o.entity_id";

/// Parses stored geometry JSON, turning a parse failure into a row error.
fn parse_geometry(json: String) -> rusqlite::Result<MapGeometry> {
    serde_json::from_str(&json).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })
}

fn map_object(row: &Row<'_>) -> rusqlite::Result<MapObject> {
    let geometry: String = row.get("geometry")?;
    let style: String = row.get("style")?;
    Ok(MapObject {
        id: row.get("id")?,
        layer_id: row.get("layer_id")?,
        // A geometry we cannot parse is a real data problem, so surface it as
        // a conversion error instead of silently inventing a position.
        geometry: parse_geometry(geometry)?,
        style: serde_json::from_str(&style).unwrap_or(Value::Object(Default::default())),
        entity_id: row.get("entity_id")?,
        entity_type: row.get("entity_type")?,
        entity_name: row.get("entity_name")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

/// Inserts the object; the geometry is serialised to JSON text here.
pub fn insert_object(conn: &Connection, o: &MapObject) -> AppResult<()> {
    conn.execute(
        "INSERT INTO map_objects (id, layer_id, kind, geometry, style, entity_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            o.id,
            o.layer_id,
            o.geometry.kind(),
            serde_json::to_string(&o.geometry)?,
            o.style.to_string(),
            o.entity_id,
            o.created_at,
            o.updated_at
        ],
    )?;
    Ok(())
}

pub fn get_object(conn: &Connection, id: &str) -> AppResult<Option<MapObject>> {
    let sql = format!("{OBJECT_SELECT} WHERE o.id = ?1");
    Ok(conn.query_row(&sql, params![id], map_object).optional()?)
}

pub fn object_exists(conn: &Connection, id: &str) -> AppResult<bool> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM map_objects WHERE id = ?1",
            params![id],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

/// All objects on all layers of a map.
pub fn list_objects(conn: &Connection, map_id: &str) -> AppResult<Vec<MapObject>> {
    let sql = format!(
        "{OBJECT_SELECT} JOIN map_layers l ON l.id = o.layer_id
         WHERE l.map_id = ?1 ORDER BY l.sort_order, o.created_at, o.id"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![map_id], map_object)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Overwrites geometry, style, entity link, layer and updated_at.
pub fn update_object(conn: &Connection, o: &MapObject) -> AppResult<()> {
    let changed = conn.execute(
        "UPDATE map_objects
         SET kind = ?2, geometry = ?3, style = ?4, entity_id = ?5, updated_at = ?6,
             layer_id = ?7
         WHERE id = ?1",
        params![
            o.id,
            o.geometry.kind(),
            serde_json::to_string(&o.geometry)?,
            o.style.to_string(),
            o.entity_id,
            o.updated_at,
            o.layer_id
        ],
    )?;
    if changed == 0 {
        return Err(AppError::NotFound(format!(
            "Map object {} does not exist",
            o.id
        )));
    }
    Ok(())
}

pub fn delete_object(conn: &Connection, id: &str) -> AppResult<()> {
    let changed = conn.execute("DELETE FROM map_objects WHERE id = ?1", params![id])?;
    if changed == 0 {
        return Err(AppError::NotFound(format!(
            "Map object {id} does not exist"
        )));
    }
    Ok(())
}

/// Labels whose geometry says they follow the path `path_id`.
/// `json_extract` reads one field out of the geometry JSON text inside SQLite.
pub fn labels_on_path(conn: &Connection, path_id: &str) -> AppResult<Vec<MapObject>> {
    let sql = format!(
        "{OBJECT_SELECT} WHERE o.kind = 'label' AND json_extract(o.geometry, '$.pathId') = ?1"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![path_id], map_object)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Ids of the path objects on one layer.
pub fn path_ids_in_layer(conn: &Connection, layer_id: &str) -> AppResult<Vec<String>> {
    let mut stmt =
        conn.prepare("SELECT id FROM map_objects WHERE layer_id = ?1 AND kind = 'path'")?;
    let rows = stmt.query_map(params![layer_id], |row| row.get(0))?;
    Ok(rows.collect::<rusqlite::Result<Vec<String>>>()?)
}

/// Maps on which `entity_id` appears as a map object (one row per object).
pub fn list_appearances(conn: &Connection, entity_id: &str) -> AppResult<Vec<MapAppearance>> {
    let mut stmt = conn.prepare(
        "SELECT m.entity_id AS map_entity_id, me.name AS map_name, o.id AS object_id, o.geometry AS geometry
         FROM map_objects o
         JOIN map_layers l ON l.id = o.layer_id
         JOIN maps m ON m.id = l.map_id
         JOIN entities me ON me.id = m.entity_id
         WHERE o.entity_id = ?1
         ORDER BY me.name COLLATE NOCASE, o.created_at, o.id",
    )?;
    let rows = stmt.query_map(params![entity_id], |row| {
        Ok(MapAppearance {
            map_entity_id: row.get("map_entity_id")?,
            map_name: row.get("map_name")?,
            object_id: row.get("object_id")?,
            geometry: parse_geometry(row.get::<_, String>("geometry")?)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Ids of existing entities of one type (used by the dev seed).
pub fn entity_ids_of_type(conn: &Connection, entity_type: &str) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare("SELECT id FROM entities WHERE type = ?1 ORDER BY id")?;
    let rows = stmt.query_map(params![entity_type], |row| row.get(0))?;
    Ok(rows.collect::<rusqlite::Result<Vec<String>>>()?)
}

pub fn entity_exists(conn: &Connection, id: &str) -> AppResult<bool> {
    Ok(conn
        .query_row("SELECT 1 FROM entities WHERE id = ?1", params![id], |_| {
            Ok(())
        })
        .optional()?
        .is_some())
}
