//! Map business rules.
//!
//! How a map comes to exist: a map is an *entity* of type `map` (so it shows
//! up in the sidebar, search and links like everything else). There is no
//! separate "add map row" step. Instead `services::entities` calls
//! [`create_for_entity`] (image maps, size 0x0 until a background is set) or
//! [`create_blank_for_entity`] (blank maps with a size and a look) inside its
//! transaction, which inserts the `maps` row and one layer called "Default".
//! All of it commits or rolls back together.
//!
//! Layers can be hidden and locked. A locked layer rejects creating,
//! changing and deleting its objects, and moving objects onto it.
//!
//! Undo/redo lives in the frontend. It relies on two things here: objects can
//! be created with a client-chosen id (so a redo recreates the same object),
//! and [`apply_ops`] applies a whole batch of changes in one transaction.
//! Layer changes (add, rename, reorder, hide, lock, delete) are NOT part of
//! the op batches: they are separate commands and not undoable in v2.

use std::path::Path;

use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::Connection;
use serde_json::Value;
use uuid::Uuid;

use crate::db;
use crate::error::{AppError, AppResult};
use crate::models::map::{
    Asset, MapAppearance, MapBackground, MapData, MapGeometry, MapInfo, MapLayer, MapObject,
    MapObjectPatch, MapOp, MapOpsResult, NewMapObject, Point,
};
use crate::services::{assets, entities};

pub const DEFAULT_LAYER_NAME: &str = "Default";

/// Allowed size of a blank map, in pixels (per side).
pub const MIN_MAP_SIZE: u32 = 256;
pub const MAX_MAP_SIZE: u32 = 16_384;
const MAX_LAYER_NAME_CHARS: usize = 60;
/// Most points a path or area may have.
pub const MAX_POINTS: usize = 20_000;
const MAX_LABEL_CHARS: usize = 200;
const MAX_ICON_CHARS: usize = 64;
pub const MAX_STYLE_BYTES: usize = 4096;
pub const MAX_OPS: usize = 5000;

pub(crate) fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn insert_default_layer(conn: &Connection, map_id: String) -> AppResult<()> {
    db::maps::insert_layer(
        conn,
        &MapLayer {
            id: Uuid::now_v7().to_string(),
            map_id,
            name: DEFAULT_LAYER_NAME.to_owned(),
            sort_order: 0,
            visible: true,
            locked: false,
        },
    )
}

/// Creates the `maps` row and the default layer for a freshly inserted
/// `map` entity. Call it inside the same transaction as the entity insert.
pub fn create_for_entity(conn: &Connection, entity_id: &str) -> AppResult<()> {
    let map_id = Uuid::now_v7().to_string();
    db::maps::insert_map(conn, &map_id, entity_id, 0, 0, None)?;
    insert_default_layer(conn, map_id)
}

/// Like [`create_for_entity`] but for a blank map: it already has its size
/// and background look. Values must have passed [`validate_blank`].
pub fn create_blank_for_entity(
    conn: &Connection,
    entity_id: &str,
    width: u32,
    height: u32,
    background: &MapBackground,
) -> AppResult<()> {
    let map_id = Uuid::now_v7().to_string();
    db::maps::insert_map(conn, &map_id, entity_id, width, height, Some(background))?;
    insert_default_layer(conn, map_id)
}

// ---- reading ------------------------------------------------------------

/// Adds the on-disk paths to the background asset, if there is one.
fn fill_paths(mut info: MapInfo, assets_dir: &Path) -> MapInfo {
    info.background = info.background.map(|a| assets::with_paths(a, assets_dir));
    info
}

fn map_by_entity(conn: &Connection, entity_id: &str) -> AppResult<MapInfo> {
    db::maps::get_map_by_entity(conn, entity_id)?
        .ok_or_else(|| AppError::NotFound(format!("No map for entity {entity_id}")))
}

/// Everything the map view needs, looked up by the map's *entity* id.
pub fn get_map(conn: &Connection, assets_dir: &Path, entity_id: &str) -> AppResult<MapData> {
    let info = map_by_entity(conn, entity_id)?;
    let layers = db::maps::list_layers(conn, &info.id)?;
    let objects = db::maps::list_objects(conn, &info.id)?;
    Ok(MapData {
        map: fill_paths(info, assets_dir),
        layers,
        objects,
    })
}

/// Records an already-copied asset in the database.
pub fn register_asset(conn: &Connection, asset: &Asset) -> AppResult<()> {
    db::assets::insert(conn, asset)
}

/// Uses `asset_id` as the map's background and resizes the map to match.
/// This also clears any blank-map background style.
pub fn set_background(
    conn: &Connection,
    assets_dir: &Path,
    entity_id: &str,
    asset_id: &str,
) -> AppResult<MapInfo> {
    let info = map_by_entity(conn, entity_id)?;
    let asset = db::assets::get(conn, asset_id)?
        .ok_or_else(|| AppError::NotFound(format!("Asset {asset_id} does not exist")))?;
    let (Some(width), Some(height)) = (asset.width, asset.height) else {
        return Err(AppError::Validation("The asset has no known size".into()));
    };
    db::maps::set_background(conn, &info.id, asset_id, width, height)?;
    let updated = db::maps::get_map(conn, &info.id)?
        .ok_or_else(|| AppError::Internal("map vanished after update".into()))?;
    Ok(fill_paths(updated, assets_dir))
}

// ---- blank maps ---------------------------------------------------------

/// `#RRGGBB` only.
fn validate_color(color: &str) -> AppResult<()> {
    let ok = color.len() == 7
        && color.starts_with('#')
        && color[1..].chars().all(|c| c.is_ascii_hexdigit());
    if ok {
        Ok(())
    } else {
        Err(AppError::Validation("Colour must look like #RRGGBB".into()))
    }
}

fn validate_size(width: u32, height: u32) -> AppResult<()> {
    let range = MIN_MAP_SIZE..=MAX_MAP_SIZE;
    if range.contains(&width) && range.contains(&height) {
        Ok(())
    } else {
        Err(AppError::Validation(format!(
            "Map size must be between {MIN_MAP_SIZE} and {MAX_MAP_SIZE} pixels per side"
        )))
    }
}

fn validate_background(background: &MapBackground) -> AppResult<()> {
    match &background.color {
        Some(color) => validate_color(color),
        None => Ok(()),
    }
}

/// Checks the inputs of a blank map. Used by `entities::create_map`.
pub fn validate_blank(width: u32, height: u32, background: &MapBackground) -> AppResult<()> {
    validate_size(width, height)?;
    validate_background(background)
}

/// Creates a map entity without an image, in one transaction (the entity
/// service also keeps the search index in sync), and returns the full map.
pub fn create_blank(
    conn: &Connection,
    assets_dir: &Path,
    name: &str,
    width: u32,
    height: u32,
    background: &MapBackground,
) -> AppResult<MapData> {
    let entity = entities::create_blank_map(conn, name, width, height, background)?;
    get_map(conn, assets_dir, &entity.id)
}

/// Blank-map operations are refused on maps that use an image.
fn require_blank(info: &MapInfo) -> AppResult<()> {
    if info.background.is_some() {
        Err(AppError::Validation(
            "This map uses an image background".into(),
        ))
    } else {
        Ok(())
    }
}

pub fn set_background_style(
    conn: &Connection,
    assets_dir: &Path,
    entity_id: &str,
    background: &MapBackground,
) -> AppResult<MapInfo> {
    let info = map_by_entity(conn, entity_id)?;
    require_blank(&info)?;
    validate_background(background)?;
    db::maps::set_background_style(conn, &info.id, background)?;
    Ok(fill_paths(map_by_entity(conn, entity_id)?, assets_dir))
}

/// Resizes a blank map. Objects outside the new size are kept (they just
/// fall off the edge).
pub fn set_size(
    conn: &Connection,
    assets_dir: &Path,
    entity_id: &str,
    width: u32,
    height: u32,
) -> AppResult<MapInfo> {
    let info = map_by_entity(conn, entity_id)?;
    require_blank(&info)?;
    validate_size(width, height)?;
    db::maps::set_size(conn, &info.id, width, height)?;
    Ok(fill_paths(map_by_entity(conn, entity_id)?, assets_dir))
}

// ---- layers -------------------------------------------------------------

fn validate_layer_name(name: &str) -> AppResult<String> {
    let trimmed = name.trim();
    let count = trimmed.chars().count();
    if count == 0 || count > MAX_LAYER_NAME_CHARS {
        return Err(AppError::Validation(format!(
            "Layer name must be 1 to {MAX_LAYER_NAME_CHARS} characters"
        )));
    }
    Ok(trimmed.to_owned())
}

fn get_layer(conn: &Connection, layer_id: &str) -> AppResult<MapLayer> {
    db::maps::get_layer(conn, layer_id)?
        .ok_or_else(|| AppError::NotFound(format!("Layer {layer_id} does not exist")))
}

/// A layer that exists and is not locked.
fn writable_layer(conn: &Connection, layer_id: &str) -> AppResult<MapLayer> {
    let layer = get_layer(conn, layer_id)?;
    if layer.locked {
        return Err(AppError::Validation(format!(
            "Layer \"{}\" is locked",
            layer.name
        )));
    }
    Ok(layer)
}

/// Adds a layer on top of the others (highest `sort_order`).
pub fn create_layer(conn: &Connection, map_entity_id: &str, name: &str) -> AppResult<MapLayer> {
    let name = validate_layer_name(name)?;
    let info = map_by_entity(conn, map_entity_id)?;
    let next = db::maps::max_layer_order(conn, &info.id)?.map_or(0, |max| max + 1);
    let layer = MapLayer {
        id: Uuid::now_v7().to_string(),
        map_id: info.id,
        name,
        sort_order: next,
        visible: true,
        locked: false,
    };
    db::maps::insert_layer(conn, &layer)?;
    Ok(layer)
}

pub fn rename_layer(conn: &Connection, layer_id: &str, name: &str) -> AppResult<MapLayer> {
    let name = validate_layer_name(name)?;
    get_layer(conn, layer_id)?;
    db::maps::rename_layer(conn, layer_id, &name)?;
    get_layer(conn, layer_id)
}

pub fn set_layer_visibility(
    conn: &Connection,
    layer_id: &str,
    visible: bool,
) -> AppResult<MapLayer> {
    get_layer(conn, layer_id)?;
    db::maps::set_layer_visible(conn, layer_id, visible)?;
    get_layer(conn, layer_id)
}

pub fn set_layer_locked(conn: &Connection, layer_id: &str, locked: bool) -> AppResult<MapLayer> {
    get_layer(conn, layer_id)?;
    db::maps::set_layer_locked(conn, layer_id, locked)?;
    get_layer(conn, layer_id)
}

/// Sets the stacking order. `layer_ids_in_order` must name exactly the map's
/// layers, each once, bottom first; `sort_order` becomes 0..n.
pub fn reorder_layers(
    conn: &Connection,
    map_entity_id: &str,
    layer_ids_in_order: &[String],
) -> AppResult<Vec<MapLayer>> {
    let info = map_by_entity(conn, map_entity_id)?;
    let existing = db::maps::list_layers(conn, &info.id)?;
    let mut wanted: Vec<&str> = layer_ids_in_order.iter().map(String::as_str).collect();
    let mut have: Vec<&str> = existing.iter().map(|l| l.id.as_str()).collect();
    // Same length and same sorted ids = exactly the same set (no extras,
    // no duplicates, nothing missing).
    wanted.sort_unstable();
    have.sort_unstable();
    if wanted != have {
        return Err(AppError::Validation(
            "The layer list must contain exactly the map's layers, each once".into(),
        ));
    }
    let tx = conn.unchecked_transaction()?;
    for (index, id) in layer_ids_in_order.iter().enumerate() {
        db::maps::set_layer_order(&tx, id, index as i32)?;
    }
    tx.commit()?;
    db::maps::list_layers(conn, &info.id)
}

/// Deletes a layer and its objects. The last layer of a map cannot be deleted.
/// Labels (on other layers) that followed a deleted path are detached.
pub fn delete_layer(conn: &Connection, layer_id: &str) -> AppResult<()> {
    let layer = get_layer(conn, layer_id)?;
    if layer.locked {
        return Err(AppError::Validation(format!(
            "Layer \"{}\" is locked",
            layer.name
        )));
    }
    if db::maps::count_layers(conn, &layer.map_id)? <= 1 {
        return Err(AppError::Validation(
            "A map needs at least one layer".into(),
        ));
    }
    let tx = conn.unchecked_transaction()?;
    for path_id in db::maps::path_ids_in_layer(&tx, layer_id)? {
        detach_labels(&tx, &path_id)?;
    }
    db::maps::delete_layer(&tx, layer_id)?;
    tx.commit()?;
    Ok(())
}

// ---- validation ---------------------------------------------------------

fn finite(values: &[f64]) -> bool {
    values.iter().all(|v| v.is_finite())
}

fn validate_points(what: &str, points: &[Point], min: usize) -> AppResult<()> {
    if points.len() < min || points.len() > MAX_POINTS {
        return Err(AppError::Validation(format!(
            "{what} needs {min} to {MAX_POINTS} points"
        )));
    }
    if points.iter().any(|p| !finite(&[p.x, p.y])) {
        return Err(AppError::Validation(format!(
            "{what} points must be finite numbers"
        )));
    }
    Ok(())
}

/// Checks everything that can be checked without the database.
fn validate_geometry(geometry: &MapGeometry) -> AppResult<()> {
    match geometry {
        MapGeometry::Pin { x, y } => {
            if !finite(&[*x, *y]) {
                return Err(AppError::Validation("Pin position must be finite".into()));
            }
        }
        MapGeometry::Path { points, .. } => validate_points("A path", points, 2)?,
        MapGeometry::Area { points } => validate_points("An area", points, 3)?,
        MapGeometry::Label {
            x,
            y,
            text,
            rotation,
            ..
        } => {
            if !finite(&[*x, *y, *rotation]) {
                return Err(AppError::Validation(
                    "Label position and rotation must be finite".into(),
                ));
            }
            let count = text.chars().count();
            if text.trim().is_empty() || count > MAX_LABEL_CHARS {
                return Err(AppError::Validation(format!(
                    "Label text must be 1 to {MAX_LABEL_CHARS} characters"
                )));
            }
        }
        MapGeometry::Stamp {
            x,
            y,
            icon,
            scale,
            rotation,
        } => {
            if !finite(&[*x, *y, *scale, *rotation]) || *scale <= 0.0 {
                return Err(AppError::Validation(
                    "Stamp position, rotation and scale must be finite and the scale positive"
                        .into(),
                ));
            }
            let icon_ok = !icon.is_empty()
                && icon.len() <= MAX_ICON_CHARS
                && icon
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
            if !icon_ok {
                return Err(AppError::Validation(format!(
                    "Stamp icon must be 1 to {MAX_ICON_CHARS} characters of a-z, 0-9 and -"
                )));
            }
        }
    }
    Ok(())
}

fn validate_style(style: &Value) -> AppResult<()> {
    if !style.is_object() {
        return Err(AppError::Validation("Style must be a JSON object".into()));
    }
    if style.to_string().len() > MAX_STYLE_BYTES {
        return Err(AppError::Validation(format!(
            "Style must be at most {MAX_STYLE_BYTES} bytes"
        )));
    }
    Ok(())
}

fn validate_entity(conn: &Connection, entity_id: &str) -> AppResult<()> {
    if db::maps::entity_exists(conn, entity_id)? {
        Ok(())
    } else {
        Err(AppError::NotFound(format!(
            "Entity {entity_id} does not exist"
        )))
    }
}

/// A label's `path_id` must name a `path` object on the same map.
fn validate_label_path(conn: &Connection, map_id: &str, geometry: &MapGeometry) -> AppResult<()> {
    let MapGeometry::Label {
        path_id: Some(path_id),
        ..
    } = geometry
    else {
        return Ok(());
    };
    let path = db::maps::get_object(conn, path_id)?
        .filter(|o| matches!(o.geometry, MapGeometry::Path { .. }))
        .ok_or_else(|| AppError::Validation(format!("Path {path_id} does not exist")))?;
    let layer = get_layer(conn, &path.layer_id)?;
    if layer.map_id != map_id {
        return Err(AppError::Validation(
            "A label can only follow a path on the same map".into(),
        ));
    }
    Ok(())
}

fn reload(conn: &Connection, id: &str) -> AppResult<MapObject> {
    db::maps::get_object(conn, id)?
        .ok_or_else(|| AppError::NotFound(format!("Map object {id} does not exist")))
}

/// Clears `path_id` on every label that follows `path_id`. Returns their ids.
/// Runs even if the label's own layer is locked (it is a side effect of
/// deleting the path, not an edit of the label).
fn detach_labels(conn: &Connection, path_id: &str) -> AppResult<Vec<String>> {
    let timestamp = now();
    let mut detached = Vec::new();
    for mut label in db::maps::labels_on_path(conn, path_id)? {
        // `&mut` borrows the geometry so we can change the field in place.
        if let MapGeometry::Label { path_id, .. } = &mut label.geometry {
            *path_id = None;
        }
        label.updated_at = timestamp.clone();
        db::maps::update_object(conn, &label)?;
        detached.push(label.id);
    }
    Ok(detached)
}

// ---- objects ------------------------------------------------------------

/// Parses a client-provided id and makes sure it is free.
fn client_id(conn: &Connection, id: &str) -> AppResult<String> {
    let parsed =
        Uuid::parse_str(id).map_err(|_| AppError::Validation("Object id must be a UUID".into()))?;
    let canonical = parsed.to_string();
    if db::maps::object_exists(conn, &canonical)? {
        return Err(AppError::Validation(format!(
            "A map object with id {canonical} already exists"
        )));
    }
    Ok(canonical)
}

/// Normalises a client-provided RFC 3339 time to our UTC millisecond format.
fn client_time(text: &str) -> AppResult<String> {
    let parsed = DateTime::parse_from_rfc3339(text)
        .map_err(|_| AppError::Validation("createdAt must be an RFC 3339 timestamp".into()))?;
    Ok(parsed
        .with_timezone(&Utc)
        .to_rfc3339_opts(SecondsFormat::Millis, true))
}

pub fn create_object(conn: &Connection, input: NewMapObject) -> AppResult<MapObject> {
    create_object_in(conn, None, input)
}

/// `map_id`: when set, the target layer must belong to that map (used by
/// op batches, which are scoped to one map).
fn create_object_in(
    conn: &Connection,
    map_id: Option<&str>,
    input: NewMapObject,
) -> AppResult<MapObject> {
    validate_geometry(&input.geometry)?;
    let style = input
        .style
        .unwrap_or_else(|| Value::Object(Default::default()));
    validate_style(&style)?;
    let layer = writable_layer(conn, &input.layer_id)?;
    if map_id.is_some_and(|m| m != layer.map_id) {
        return Err(AppError::Validation(
            "The layer belongs to a different map".into(),
        ));
    }
    validate_label_path(conn, &layer.map_id, &input.geometry)?;
    if let Some(entity_id) = &input.entity_id {
        validate_entity(conn, entity_id)?;
    }
    let id = match &input.id {
        Some(id) => client_id(conn, id)?,
        None => Uuid::now_v7().to_string(),
    };
    let timestamp = now();
    let created_at = match &input.created_at {
        Some(text) => client_time(text)?,
        None => timestamp.clone(),
    };
    db::maps::insert_object(
        conn,
        &MapObject {
            id: id.clone(),
            layer_id: input.layer_id,
            geometry: input.geometry,
            style,
            entity_id: input.entity_id,
            entity_type: None,
            entity_name: None,
            created_at,
            updated_at: timestamp,
        },
    )?;
    // Reload so the entity type/name from the join are filled in.
    reload(conn, &id)
}

pub fn update_object(conn: &Connection, id: &str, patch: MapObjectPatch) -> AppResult<MapObject> {
    update_object_in(conn, None, id, patch)
}

fn update_object_in(
    conn: &Connection,
    map_id: Option<&str>,
    id: &str,
    patch: MapObjectPatch,
) -> AppResult<MapObject> {
    let mut object = reload(conn, id)?;
    // The layer the object is on now must be unlocked.
    let current = db::maps::get_layer_for_object(conn, id)?
        .ok_or_else(|| AppError::Internal("map object has no layer".into()))?;
    if current.locked {
        return Err(AppError::Validation(format!(
            "Layer \"{}\" is locked",
            current.name
        )));
    }
    if map_id.is_some_and(|m| m != current.map_id) {
        return Err(AppError::Validation(
            "The object belongs to a different map".into(),
        ));
    }
    if let Some(geometry) = patch.geometry {
        validate_geometry(&geometry)?;
        // An object keeps its kind: a pin cannot turn into a path, etc.
        if geometry.kind() != object.geometry.kind() {
            return Err(AppError::Validation(format!(
                "Cannot change a {} into a {}",
                object.geometry.kind(),
                geometry.kind()
            )));
        }
        object.geometry = geometry;
    }
    if let Some(style) = patch.style {
        validate_style(&style)?;
        object.style = style;
    }
    if patch.unlink_entity {
        object.entity_id = None;
    } else if let Some(entity_id) = patch.entity_id {
        validate_entity(conn, &entity_id)?;
        object.entity_id = Some(entity_id);
    }
    if let Some(layer_id) = patch.layer_id
        && layer_id != object.layer_id
    {
        let target = writable_layer(conn, &layer_id)?;
        if target.map_id != current.map_id {
            return Err(AppError::Validation(
                "Objects can only move between layers of the same map".into(),
            ));
        }
        object.layer_id = layer_id;
    }
    validate_label_path(conn, &current.map_id, &object.geometry)?;
    object.updated_at = now();
    db::maps::update_object(conn, &object)?;
    reload(conn, id)
}

pub fn delete_object(conn: &Connection, id: &str) -> AppResult<()> {
    delete_object_in(conn, None, id).map(|_| ())
}

/// Deletes an object; returns the ids of labels detached from it (if it was
/// a path).
fn delete_object_in(conn: &Connection, map_id: Option<&str>, id: &str) -> AppResult<Vec<String>> {
    let layer = db::maps::get_layer_for_object(conn, id)?
        .ok_or_else(|| AppError::NotFound(format!("Map object {id} does not exist")))?;
    if layer.locked {
        return Err(AppError::Validation(format!(
            "Layer \"{}\" is locked",
            layer.name
        )));
    }
    if map_id.is_some_and(|m| m != layer.map_id) {
        return Err(AppError::Validation(
            "The object belongs to a different map".into(),
        ));
    }
    let detached = detach_labels(conn, id)?;
    db::maps::delete_object(conn, id)?;
    Ok(detached)
}

/// Applies a batch of create/update/delete ops to one map, all-or-nothing:
/// if any op fails the whole batch is rolled back and the error is returned.
/// Ops run in order, so a later op may refer to an object created earlier in
/// the same batch.
pub fn apply_ops(
    conn: &Connection,
    map_entity_id: &str,
    ops: Vec<MapOp>,
) -> AppResult<MapOpsResult> {
    let info = map_by_entity(conn, map_entity_id)?;
    if ops.len() > MAX_OPS {
        return Err(AppError::Validation(format!(
            "At most {MAX_OPS} operations per batch"
        )));
    }
    // If an early `?` returns, the transaction is dropped without commit
    // and SQLite rolls everything back.
    let tx = conn.unchecked_transaction()?;
    let mut touched: Vec<String> = Vec::new();
    let mut deleted_ids: Vec<String> = Vec::new();
    let mut detached_ids: Vec<String> = Vec::new();
    for op in ops {
        match op {
            MapOp::Create { object } => {
                let created = create_object_in(&tx, Some(&info.id), object)?;
                touched.push(created.id);
            }
            MapOp::Update { id, patch } => {
                update_object_in(&tx, Some(&info.id), &id, patch)?;
                touched.push(id);
            }
            MapOp::Delete { id } => {
                detached_ids.extend(delete_object_in(&tx, Some(&info.id), &id)?);
                deleted_ids.push(id);
            }
        }
    }
    // Final state of what is still there, without duplicates.
    let mut seen = std::collections::HashSet::new();
    let mut objects = Vec::new();
    for id in touched {
        if seen.insert(id.clone())
            && let Some(object) = db::maps::get_object(&tx, &id)?
        {
            objects.push(object);
        }
    }
    let mut detached = Vec::new();
    for id in detached_ids {
        if let Some(object) = db::maps::get_object(&tx, &id)? {
            detached.push(object);
        }
    }
    tx.commit()?;
    Ok(MapOpsResult {
        objects,
        deleted_ids,
        detached,
    })
}

pub fn list_appearances(conn: &Connection, entity_id: &str) -> AppResult<Vec<MapAppearance>> {
    db::maps::list_appearances(conn, entity_id)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::db::migrations::MIGRATIONS;
    use crate::models::entity::NewEntity;
    use crate::services::entities;
    use serde_json::json;
    use std::path::PathBuf;

    fn conn() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        c.pragma_update(None, "foreign_keys", "ON").unwrap();
        MIGRATIONS.to_latest(&mut c).unwrap();
        c
    }

    fn make(c: &Connection, entity_type: &str, name: &str) -> String {
        entities::create(
            c,
            NewEntity {
                entity_type: entity_type.into(),
                name: name.into(),
            },
        )
        .unwrap()
        .id
    }

    fn pin(layer_id: &str, x: f64, y: f64, entity_id: Option<String>) -> NewMapObject {
        NewMapObject {
            id: None,
            layer_id: layer_id.into(),
            geometry: MapGeometry::Pin { x, y },
            style: None,
            entity_id,
            created_at: None,
        }
    }

    fn dir() -> PathBuf {
        PathBuf::from("assets")
    }

    #[test]
    fn geometry_json_round_trip() {
        let g = MapGeometry::Pin { x: 12.5, y: -3.0 };
        let text = serde_json::to_string(&g).unwrap();
        assert_eq!(text, r#"{"kind":"pin","x":12.5,"y":-3.0}"#);
        assert_eq!(serde_json::from_str::<MapGeometry>(&text).unwrap(), g);
        assert!(serde_json::from_str::<MapGeometry>(r#"{"kind":"nope"}"#).is_err());

        // And through the database.
        let c = conn();
        let map = make(&c, "map", "World");
        let data = get_map(&c, &dir(), &map).unwrap();
        let obj = create_object(&c, pin(&data.layers[0].id, 10.25, 99.0, None)).unwrap();
        assert_eq!(obj.geometry, MapGeometry::Pin { x: 10.25, y: 99.0 });
        let moved = update_object(
            &c,
            &obj.id,
            MapObjectPatch {
                geometry: Some(MapGeometry::Pin { x: 1.0, y: 2.0 }),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(moved.geometry, MapGeometry::Pin { x: 1.0, y: 2.0 });
        assert_eq!(
            get_map(&c, &dir(), &map).unwrap().objects[0].geometry,
            moved.geometry
        );
    }

    #[test]
    fn deleting_an_entity_unlinks_but_keeps_its_pins() {
        let c = conn();
        let map = make(&c, "map", "World");
        let town = make(&c, "location", "Town");
        let layer = get_map(&c, &dir(), &map).unwrap().layers[0].id.clone();
        let obj = create_object(&c, pin(&layer, 5.0, 5.0, Some(town.clone()))).unwrap();
        assert_eq!(obj.entity_name.as_deref(), Some("Town"));
        assert_eq!(obj.entity_type.as_deref(), Some("location"));

        entities::delete(&c, &town).unwrap();
        let data = get_map(&c, &dir(), &map).unwrap();
        assert_eq!(data.objects.len(), 1);
        assert_eq!(data.objects[0].entity_id, None);
    }

    #[test]
    fn deleting_the_map_cascades_to_objects() {
        let c = conn();
        let map = make(&c, "map", "World");
        let layer = get_map(&c, &dir(), &map).unwrap().layers[0].id.clone();
        create_object(&c, pin(&layer, 1.0, 1.0, None)).unwrap();
        entities::delete(&c, &map).unwrap();
        let n: i64 = c
            .query_row("SELECT count(*) FROM map_objects", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn lists_appearances_and_validates_input() {
        let c = conn();
        let map_a = make(&c, "map", "Alpha");
        let map_b = make(&c, "map", "Beta");
        let town = make(&c, "location", "Town");
        for m in [&map_a, &map_b] {
            let layer = get_map(&c, &dir(), m).unwrap().layers[0].id.clone();
            create_object(&c, pin(&layer, 7.0, 8.0, Some(town.clone()))).unwrap();
        }
        let apps = list_appearances(&c, &town).unwrap();
        assert_eq!(apps.len(), 2);
        assert_eq!(apps[0].map_name, "Alpha");
        assert_eq!(apps[0].map_entity_id, map_a);
        assert_eq!(apps[1].geometry, MapGeometry::Pin { x: 7.0, y: 8.0 });

        let layer = get_map(&c, &dir(), &map_a).unwrap().layers[0].id.clone();
        assert!(matches!(
            create_object(&c, pin("nope", 0.0, 0.0, None)),
            Err(AppError::NotFound(_))
        ));
        assert!(matches!(
            create_object(&c, pin(&layer, 0.0, 0.0, Some("nope".into()))),
            Err(AppError::NotFound(_))
        ));
        assert!(matches!(
            create_object(&c, pin(&layer, f64::NAN, 0.0, None)),
            Err(AppError::Validation(_))
        ));
        let mut bad = pin(&layer, 0.0, 0.0, None);
        bad.style = Some(json!([1]));
        assert!(matches!(
            create_object(&c, bad),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn setting_background_updates_size() {
        let c = conn();
        let map = make(&c, "map", "World");
        let asset = Asset {
            id: "asset-1".into(),
            file_name: "m.png".into(),
            mime: "image/png".into(),
            width: Some(800),
            height: Some(600),
            created_at: now(),
            path: String::new(),
            thumb_path: None,
        };
        register_asset(&c, &asset).unwrap();
        let info = set_background(&c, &dir(), &map, "asset-1").unwrap();
        assert_eq!((info.width, info.height), (800, 600));
        let bg = info.background.unwrap();
        assert!(bg.path.ends_with("asset-1.png"));
        assert!(matches!(
            set_background(&c, &dir(), &map, "missing"),
            Err(AppError::NotFound(_))
        ));
    }

    // ---- Phase 10 tests -------------------------------------------------

    use crate::models::map::BackgroundKind;

    fn bg(kind: BackgroundKind, color: Option<&str>) -> MapBackground {
        MapBackground {
            kind,
            color: color.map(Into::into),
        }
    }

    fn pts(coords: &[(f64, f64)]) -> Vec<Point> {
        coords.iter().map(|&(x, y)| Point { x, y }).collect()
    }

    fn shape(layer: &str, geometry: MapGeometry) -> NewMapObject {
        NewMapObject {
            id: None,
            layer_id: layer.into(),
            geometry,
            style: None,
            entity_id: None,
            created_at: None,
        }
    }

    fn path_geo() -> MapGeometry {
        MapGeometry::Path {
            points: pts(&[(0.0, 0.0), (10.0, 5.0), (20.0, 0.0)]),
            closed: false,
        }
    }

    fn label_geo(path_id: Option<String>) -> MapGeometry {
        MapGeometry::Label {
            x: 5.0,
            y: 6.0,
            text: "River".into(),
            rotation: 0.0,
            path_id,
        }
    }

    /// A map with its default layer id.
    fn map_and_layer(c: &Connection) -> (String, String) {
        let map = make(c, "map", "World");
        let layer = get_map(c, &dir(), &map).unwrap().layers[0].id.clone();
        (map, layer)
    }

    fn is_validation<T>(r: AppResult<T>) -> bool {
        matches!(r, Err(AppError::Validation(_)))
    }

    fn move_to(layer: &str) -> MapObjectPatch {
        MapObjectPatch {
            layer_id: Some(layer.into()),
            ..Default::default()
        }
    }

    #[test]
    fn creates_a_blank_map_in_one_step() {
        let c = conn();
        let data = create_blank(
            &c,
            &dir(),
            "Blank",
            1024,
            768,
            &bg(BackgroundKind::Parchment, Some("#e8d9b5")),
        )
        .unwrap();
        assert_eq!((data.map.width, data.map.height), (1024, 768));
        assert!(data.map.background.is_none());
        assert_eq!(
            data.map.background_style,
            Some(bg(BackgroundKind::Parchment, Some("#e8d9b5")))
        );
        assert_eq!(data.layers.len(), 1);
        assert_eq!(data.layers[0].name, DEFAULT_LAYER_NAME);
        assert!(!data.layers[0].locked);
        // It is a normal, searchable map entity.
        assert!(
            !crate::services::search::search(&c, "Blank", 5)
                .unwrap()
                .is_empty()
        );
        // A plain map entity still has no style.
        let plain = make(&c, "map", "Image map");
        assert!(
            get_map(&c, &dir(), &plain)
                .unwrap()
                .map
                .background_style
                .is_none()
        );
    }

    #[test]
    fn blank_map_validation_creates_nothing() {
        let c = conn();
        let ok = bg(BackgroundKind::Ocean, None);
        for (w, h) in [(255, 1000), (1000, 255), (16385, 1000), (1000, 16385)] {
            assert!(is_validation(create_blank(&c, &dir(), "X", w, h, &ok)));
        }
        for color in ["red", "#12345", "#12345g", "123456", "#1234567"] {
            let bad = bg(BackgroundKind::Plain, Some(color));
            assert!(is_validation(create_blank(&c, &dir(), "X", 512, 512, &bad)));
        }
        assert!(is_validation(create_blank(&c, &dir(), "  ", 512, 512, &ok)));
        let n: i64 = c
            .query_row("SELECT count(*) FROM entities", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
        // Edges are allowed.
        create_blank(&c, &dir(), "Small", 256, 256, &ok).unwrap();
        create_blank(&c, &dir(), "Big", 16384, 16384, &ok).unwrap();
    }

    #[test]
    fn background_style_and_size_only_change_on_blank_maps() {
        let c = conn();
        let data = create_blank(
            &c,
            &dir(),
            "Blank",
            512,
            512,
            &bg(BackgroundKind::Plain, Some("#ffffff")),
        )
        .unwrap();
        let id = data.map.entity_id;
        let info = set_background_style(&c, &dir(), &id, &bg(BackgroundKind::Ocean, None)).unwrap();
        assert_eq!(info.background_style, Some(bg(BackgroundKind::Ocean, None)));
        assert!(is_validation(set_background_style(
            &c,
            &dir(),
            &id,
            &bg(BackgroundKind::Plain, Some("nope"))
        )));
        let info = set_size(&c, &dir(), &id, 2000, 1000).unwrap();
        assert_eq!((info.width, info.height), (2000, 1000));
        assert!(is_validation(set_size(&c, &dir(), &id, 10, 1000)));

        // Once an image is the background, both are refused and the style is cleared.
        let asset = Asset {
            id: "asset-1".into(),
            file_name: "m.png".into(),
            mime: "image/png".into(),
            width: Some(800),
            height: Some(600),
            created_at: now(),
            path: String::new(),
            thumb_path: None,
        };
        register_asset(&c, &asset).unwrap();
        let info = set_background(&c, &dir(), &id, "asset-1").unwrap();
        assert!(info.background_style.is_none());
        assert!(is_validation(set_size(&c, &dir(), &id, 512, 512)));
        assert!(is_validation(set_background_style(
            &c,
            &dir(),
            &id,
            &bg(BackgroundKind::Ocean, None)
        )));
    }

    #[test]
    fn layers_can_be_created_renamed_hidden_and_locked() {
        let c = conn();
        let (map, first) = map_and_layer(&c);
        let borders = create_layer(&c, &map, "  Borders ").unwrap();
        assert_eq!(borders.name, "Borders");
        assert_eq!(borders.sort_order, 1);
        assert!(borders.visible && !borders.locked);
        let third = create_layer(&c, &map, "Labels").unwrap();
        assert_eq!(third.sort_order, 2);
        assert!(is_validation(create_layer(&c, &map, "   ")));
        assert!(is_validation(create_layer(&c, &map, &"x".repeat(61))));
        assert!(matches!(
            create_layer(&c, "nope", "A"),
            Err(AppError::NotFound(_))
        ));

        assert_eq!(rename_layer(&c, &first, "Land").unwrap().name, "Land");
        assert!(is_validation(rename_layer(&c, &first, "")));
        assert!(!set_layer_visibility(&c, &first, false).unwrap().visible);
        assert!(set_layer_locked(&c, &first, true).unwrap().locked);
        let layers = get_map(&c, &dir(), &map).unwrap().layers;
        assert_eq!(layers.len(), 3);
        assert!(!layers[0].visible && layers[0].locked);
        assert!(matches!(
            set_layer_locked(&c, "nope", true),
            Err(AppError::NotFound(_))
        ));
    }

    #[test]
    fn reordering_layers_needs_exactly_the_maps_layers() {
        let c = conn();
        let (map, a) = map_and_layer(&c);
        let b = create_layer(&c, &map, "B").unwrap().id;
        let d = create_layer(&c, &map, "C").unwrap().id;
        let other = map_and_layer(&c).1;

        let out = reorder_layers(&c, &map, &[d.clone(), a.clone(), b.clone()]).unwrap();
        let ids: Vec<&str> = out.iter().map(|l| l.id.as_str()).collect();
        assert_eq!(ids, [d.as_str(), a.as_str(), b.as_str()]);
        assert_eq!(
            out.iter().map(|l| l.sort_order).collect::<Vec<_>>(),
            [0, 1, 2]
        );
        assert_eq!(get_map(&c, &dir(), &map).unwrap().layers[0].id, d);

        // Missing, extra, duplicate and foreign ids are all refused.
        assert!(is_validation(reorder_layers(
            &c,
            &map,
            &[a.clone(), b.clone()]
        )));
        assert!(is_validation(reorder_layers(
            &c,
            &map,
            &[a.clone(), b.clone(), d.clone(), other.clone()]
        )));
        assert!(is_validation(reorder_layers(
            &c,
            &map,
            &[a.clone(), a.clone(), b.clone()]
        )));
        assert!(is_validation(reorder_layers(&c, &map, &[a, b, other])));
        // Nothing changed after the refusals.
        assert_eq!(get_map(&c, &dir(), &map).unwrap().layers[0].id, d);
    }

    #[test]
    fn deleting_layers_cascades_but_never_removes_the_last() {
        let c = conn();
        let (map, first) = map_and_layer(&c);
        let second = create_layer(&c, &map, "Two").unwrap().id;
        create_object(&c, pin(&second, 1.0, 1.0, None)).unwrap();
        delete_layer(&c, &second).unwrap();
        let data = get_map(&c, &dir(), &map).unwrap();
        assert_eq!(data.layers.len(), 1);
        assert!(data.objects.is_empty());
        assert!(is_validation(delete_layer(&c, &first)));
        assert!(matches!(
            delete_layer(&c, "nope"),
            Err(AppError::NotFound(_))
        ));
        // A locked layer cannot be deleted either.
        let third = create_layer(&c, &map, "Three").unwrap().id;
        set_layer_locked(&c, &third, true).unwrap();
        assert!(is_validation(delete_layer(&c, &third)));
    }

    #[test]
    fn deleting_a_layer_detaches_labels_that_followed_its_paths() {
        let c = conn();
        let (map, first) = map_and_layer(&c);
        let second = create_layer(&c, &map, "Rivers").unwrap().id;
        let river = create_object(&c, shape(&second, path_geo())).unwrap();
        let label = create_object(&c, shape(&first, label_geo(Some(river.id)))).unwrap();
        delete_layer(&c, &second).unwrap();
        let after = get_map(&c, &dir(), &map).unwrap();
        assert_eq!(after.objects.len(), 1);
        assert_eq!(after.objects[0].id, label.id);
        assert_eq!(after.objects[0].geometry, label_geo(None));
    }

    #[test]
    fn locked_layers_reject_object_changes() {
        let c = conn();
        let (map, first) = map_and_layer(&c);
        let second = create_layer(&c, &map, "Two").unwrap().id;
        let obj = create_object(&c, pin(&first, 1.0, 1.0, None)).unwrap();

        set_layer_locked(&c, &first, true).unwrap();
        assert!(is_validation(create_object(
            &c,
            pin(&first, 2.0, 2.0, None)
        )));
        assert!(is_validation(update_object(
            &c,
            &obj.id,
            MapObjectPatch {
                geometry: Some(MapGeometry::Pin { x: 5.0, y: 5.0 }),
                ..Default::default()
            }
        )));
        assert!(is_validation(delete_object(&c, &obj.id)));
        // Even moving it off a locked layer is an edit of that layer.
        assert!(is_validation(update_object(&c, &obj.id, move_to(&second))));
        assert_eq!(get_map(&c, &dir(), &map).unwrap().objects.len(), 1);

        // Unlocked again: everything works, and moving between layers too.
        set_layer_locked(&c, &first, false).unwrap();
        let moved = update_object(&c, &obj.id, move_to(&second)).unwrap();
        assert_eq!(moved.layer_id, second);

        // Moving onto a locked layer is refused.
        set_layer_locked(&c, &first, true).unwrap();
        assert!(is_validation(update_object(&c, &obj.id, move_to(&first))));
        // Hidden layers are still editable.
        set_layer_visibility(&c, &second, false).unwrap();
        delete_object(&c, &obj.id).unwrap();
    }

    #[test]
    fn objects_cannot_move_to_another_maps_layer() {
        let c = conn();
        let (_m1, l1) = map_and_layer(&c);
        let (_m2, l2) = map_and_layer(&c);
        let obj = create_object(&c, pin(&l1, 1.0, 1.0, None)).unwrap();
        assert!(is_validation(update_object(&c, &obj.id, move_to(&l2))));
        assert!(matches!(
            update_object(&c, &obj.id, move_to("nope")),
            Err(AppError::NotFound(_))
        ));
    }

    #[test]
    fn every_geometry_variant_round_trips_as_json_and_through_the_database() {
        let variants = vec![
            MapGeometry::Pin { x: 1.5, y: 2.0 },
            MapGeometry::Path {
                points: pts(&[(0.0, 0.5), (3.0, 4.0)]),
                closed: true,
            },
            MapGeometry::Area {
                points: pts(&[(0.0, 0.0), (10.0, 0.0), (5.0, 8.0)]),
            },
            label_geo(None),
            MapGeometry::Stamp {
                x: 9.0,
                y: 8.0,
                icon: "castle-2".into(),
                scale: 1.5,
                rotation: 30.0,
            },
        ];
        let c = conn();
        let (map, layer) = map_and_layer(&c);
        for g in variants {
            let text = serde_json::to_string(&g).unwrap();
            assert_eq!(serde_json::from_str::<MapGeometry>(&text).unwrap(), g);
            let created = create_object(&c, shape(&layer, g.clone())).unwrap();
            assert_eq!(created.geometry, g);
        }
        let data = get_map(&c, &dir(), &map).unwrap();
        let kinds: Vec<&str> = data.objects.iter().map(|o| o.geometry.kind()).collect();
        assert_eq!(kinds, ["pin", "path", "area", "label", "stamp"]);
        // The stored `kind` column matches the geometry.
        let stored: Vec<String> = c
            .prepare("SELECT kind FROM map_objects ORDER BY created_at, id")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(stored, kinds);
        // Wire format of the new variants.
        let json = serde_json::to_value(label_geo(Some("p".into()))).unwrap();
        assert_eq!(
            json,
            json!({"kind":"label","x":5.0,"y":6.0,"text":"River","rotation":0.0,"pathId":"p"})
        );
        let area = serde_json::to_value(MapGeometry::Area {
            points: pts(&[(1.0, 2.0)]),
        })
        .unwrap();
        assert_eq!(area, json!({"kind":"area","points":[{"x":1.0,"y":2.0}]}));
        // `pathId` may be omitted.
        let parsed: MapGeometry =
            serde_json::from_str(r#"{"kind":"label","x":1,"y":2,"text":"a","rotation":0}"#)
                .unwrap();
        assert_eq!(
            parsed,
            MapGeometry::Label {
                x: 1.0,
                y: 2.0,
                text: "a".into(),
                rotation: 0.0,
                path_id: None
            }
        );
    }

    #[test]
    fn geometry_validation_rejects_bad_shapes() {
        let c = conn();
        let (_map, layer) = map_and_layer(&c);
        let try_create = |g: MapGeometry| create_object(&c, shape(&layer, g));
        let nan = f64::NAN;
        let long_path: Vec<Point> = (0..=MAX_POINTS)
            .map(|i| Point {
                x: i as f64,
                y: 0.0,
            })
            .collect();
        let label = |text: &str, y: f64| MapGeometry::Label {
            x: 0.0,
            y,
            text: text.into(),
            rotation: 0.0,
            path_id: None,
        };
        let bad = vec![
            MapGeometry::Path {
                points: pts(&[(0.0, 0.0)]),
                closed: false,
            },
            MapGeometry::Path {
                points: pts(&[(0.0, 0.0), (nan, 1.0)]),
                closed: false,
            },
            MapGeometry::Path {
                points: long_path,
                closed: false,
            },
            MapGeometry::Area {
                points: pts(&[(0.0, 0.0), (1.0, 1.0)]),
            },
            MapGeometry::Area {
                points: pts(&[(0.0, 0.0), (1.0, 1.0), (f64::INFINITY, 2.0)]),
            },
            label("", 0.0),
            label("   ", 0.0),
            label(&"x".repeat(201), 0.0),
            label("ok", nan),
        ];
        for g in bad {
            assert!(is_validation(try_create(g.clone())), "{g:?}");
        }
        let stamp = |icon: &str, scale: f64| MapGeometry::Stamp {
            x: 0.0,
            y: 0.0,
            icon: icon.into(),
            scale,
            rotation: 0.0,
        };
        for g in [
            stamp("", 1.0),
            stamp("Castle", 1.0),
            stamp("a b", 1.0),
            stamp("a_b", 1.0),
            stamp(&"a".repeat(65), 1.0),
            stamp("castle", 0.0),
            stamp("castle", -1.0),
            stamp("castle", nan),
        ] {
            assert!(is_validation(try_create(g.clone())), "{g:?}");
        }
        // Boundaries that are fine.
        assert!(try_create(stamp(&"a".repeat(64), 0.1)).is_ok());
        assert!(try_create(label(&"x".repeat(200), 0.0)).is_ok());
        // The same checks apply to updates, and kinds cannot change.
        let pin_obj = create_object(&c, pin(&layer, 1.0, 1.0, None)).unwrap();
        for geometry in [path_geo(), MapGeometry::Pin { x: nan, y: 0.0 }] {
            assert!(is_validation(update_object(
                &c,
                &pin_obj.id,
                MapObjectPatch {
                    geometry: Some(geometry),
                    ..Default::default()
                }
            )));
        }
    }

    #[test]
    fn style_must_be_a_small_object() {
        let c = conn();
        let (_map, layer) = map_and_layer(&c);
        let mut input = shape(&layer, path_geo());
        input.style = Some(json!({ "stroke": "#112233", "dash": [4, 2] }));
        let obj = create_object(&c, input).unwrap();
        assert_eq!(obj.style["dash"], json!([4, 2]));
        let mut big = shape(&layer, path_geo());
        big.style = Some(json!({ "note": "x".repeat(5000) }));
        assert!(is_validation(create_object(&c, big)));
        assert!(is_validation(update_object(
            &c,
            &obj.id,
            MapObjectPatch {
                style: Some(json!("nope")),
                ..Default::default()
            }
        )));
    }

    #[test]
    fn labels_follow_existing_paths_on_the_same_map_and_detach_on_delete() {
        let c = conn();
        let (map, layer) = map_and_layer(&c);
        let river = create_object(&c, shape(&layer, path_geo())).unwrap();
        let label = create_object(&c, shape(&layer, label_geo(Some(river.id.clone())))).unwrap();
        let free = create_object(&c, shape(&layer, label_geo(None))).unwrap();
        let pin_obj = create_object(&c, pin(&layer, 1.0, 1.0, None)).unwrap();

        // The path must exist, be a path, and be on the same map.
        assert!(is_validation(create_object(
            &c,
            shape(&layer, label_geo(Some("nope".into())))
        )));
        assert!(is_validation(create_object(
            &c,
            shape(&layer, label_geo(Some(pin_obj.id.clone())))
        )));
        let (_other_map, other_layer) = map_and_layer(&c);
        assert!(is_validation(create_object(
            &c,
            shape(&other_layer, label_geo(Some(river.id.clone())))
        )));
        // Attaching later through an update works too.
        let attached = update_object(
            &c,
            &free.id,
            MapObjectPatch {
                geometry: Some(label_geo(Some(river.id.clone()))),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(attached.geometry, label_geo(Some(river.id.clone())));

        // Deleting the path keeps both labels, at their old x/y, detached.
        delete_object(&c, &river.id).unwrap();
        let data = get_map(&c, &dir(), &map).unwrap();
        let labels: Vec<_> = data
            .objects
            .iter()
            .filter(|o| o.geometry.kind() == "label")
            .collect();
        assert_eq!(labels.len(), 2);
        assert!(labels.iter().all(|l| l.geometry == label_geo(None)));
        assert!(labels.iter().any(|l| l.id == label.id));
    }

    #[test]
    fn client_provided_ids_are_used_and_must_be_unique_uuids() {
        let c = conn();
        let (_map, layer) = map_and_layer(&c);
        let id = Uuid::now_v7().to_string();
        let mut input = pin(&layer, 1.0, 2.0, None);
        input.id = Some(id.clone());
        input.created_at = Some("2020-01-02T03:04:05+01:00".into());
        let obj = create_object(&c, input.clone()).unwrap();
        assert_eq!(obj.id, id);
        assert_eq!(obj.created_at, "2020-01-02T02:04:05.000Z");

        // Same id again: refused, and the original is untouched.
        assert!(is_validation(create_object(&c, input.clone())));
        // Delete then recreate with the same id (undo of a delete).
        delete_object(&c, &id).unwrap();
        assert_eq!(create_object(&c, input.clone()).unwrap().id, id);

        let mut bad = input.clone();
        bad.id = Some("not-a-uuid".into());
        assert!(is_validation(create_object(&c, bad)));
        let mut bad = pin(&layer, 1.0, 2.0, None);
        bad.created_at = Some("yesterday".into());
        assert!(is_validation(create_object(&c, bad)));
    }

    fn create_op(layer: &str, id: Option<String>, x: f64) -> MapOp {
        let mut object = pin(layer, x, 0.0, None);
        object.id = id;
        MapOp::Create { object }
    }

    #[test]
    fn apply_ops_runs_a_batch_in_order_and_reports_the_result() {
        let c = conn();
        let (map, layer) = map_and_layer(&c);
        let keep = create_object(&c, pin(&layer, 1.0, 1.0, None)).unwrap();
        let gone = create_object(&c, pin(&layer, 2.0, 2.0, None)).unwrap();
        let river_id = Uuid::now_v7().to_string();
        let label_id = Uuid::now_v7().to_string();
        let result = apply_ops(
            &c,
            &map,
            vec![
                // A path and a label that follows it, created in one batch.
                MapOp::Create {
                    object: NewMapObject {
                        id: Some(river_id.clone()),
                        ..shape(&layer, path_geo())
                    },
                },
                MapOp::Create {
                    object: NewMapObject {
                        id: Some(label_id.clone()),
                        ..shape(&layer, label_geo(Some(river_id.clone())))
                    },
                },
                MapOp::Update {
                    id: keep.id.clone(),
                    patch: MapObjectPatch {
                        geometry: Some(MapGeometry::Pin { x: 50.0, y: 60.0 }),
                        ..Default::default()
                    },
                },
                MapOp::Delete {
                    id: gone.id.clone(),
                },
            ],
        )
        .unwrap();
        let ids: Vec<&str> = result.objects.iter().map(|o| o.id.as_str()).collect();
        assert_eq!(
            ids,
            [river_id.as_str(), label_id.as_str(), keep.id.as_str()]
        );
        assert_eq!(result.deleted_ids, std::slice::from_ref(&gone.id));
        assert!(result.detached.is_empty());
        let data = get_map(&c, &dir(), &map).unwrap();
        assert_eq!(data.objects.len(), 3);

        // Deleting the path in a batch reports the detached label.
        let result = apply_ops(&c, &map, vec![MapOp::Delete { id: river_id }]).unwrap();
        assert_eq!(result.detached.len(), 1);
        assert_eq!(result.detached[0].id, label_id);
        assert_eq!(result.detached[0].geometry, label_geo(None));
    }

    #[test]
    fn a_failing_op_rolls_back_the_whole_batch() {
        let c = conn();
        let (map, layer) = map_and_layer(&c);
        let existing = create_object(&c, pin(&layer, 1.0, 1.0, None)).unwrap();
        let before = get_map(&c, &dir(), &map).unwrap().objects;
        let dup = Uuid::now_v7().to_string();
        let foreign_layer = map_and_layer(&c).1;

        let failing: Vec<Vec<MapOp>> = vec![
            // Update of a missing object, after a create and a delete.
            vec![
                create_op(&layer, None, 5.0),
                MapOp::Delete {
                    id: existing.id.clone(),
                },
                MapOp::Update {
                    id: "missing".into(),
                    patch: MapObjectPatch::default(),
                },
            ],
            // Invalid geometry in the last op.
            vec![
                create_op(&layer, None, 5.0),
                MapOp::Create {
                    object: shape(
                        &layer,
                        MapGeometry::Area {
                            points: pts(&[(0.0, 0.0)]),
                        },
                    ),
                },
            ],
            // Duplicate client id within the batch.
            vec![
                create_op(&layer, Some(dup.clone()), 1.0),
                create_op(&layer, Some(dup), 2.0),
            ],
            // A layer from another map.
            vec![
                create_op(&layer, None, 1.0),
                create_op(&foreign_layer, None, 2.0),
            ],
        ];
        for ops in failing {
            assert!(apply_ops(&c, &map, ops).is_err());
            assert_eq!(get_map(&c, &dir(), &map).unwrap().objects, before);
        }

        // A locked layer rejects the batch as well.
        set_layer_locked(&c, &layer, true).unwrap();
        assert!(is_validation(apply_ops(
            &c,
            &map,
            vec![MapOp::Delete { id: existing.id }]
        )));
        assert_eq!(get_map(&c, &dir(), &map).unwrap().objects, before);
        // Too many ops.
        let many: Vec<MapOp> = (0..=MAX_OPS)
            .map(|_| MapOp::Delete { id: "x".into() })
            .collect();
        assert!(is_validation(apply_ops(&c, &map, many)));
        assert!(matches!(
            apply_ops(&c, "nope", vec![]),
            Err(AppError::NotFound(_))
        ));
    }

    #[test]
    fn ops_can_restore_an_undone_delete_with_the_same_id_and_order() {
        let c = conn();
        let (map, layer) = map_and_layer(&c);
        let first = create_object(&c, pin(&layer, 1.0, 1.0, None)).unwrap();
        let second = create_object(&c, pin(&layer, 2.0, 2.0, None)).unwrap();
        apply_ops(
            &c,
            &map,
            vec![MapOp::Delete {
                id: first.id.clone(),
            }],
        )
        .unwrap();
        // Undo: recreate with the old id and creation time.
        apply_ops(
            &c,
            &map,
            vec![MapOp::Create {
                object: NewMapObject {
                    id: Some(first.id.clone()),
                    created_at: Some(first.created_at.clone()),
                    ..pin(&layer, 1.0, 1.0, None)
                },
            }],
        )
        .unwrap();
        let ids: Vec<String> = get_map(&c, &dir(), &map)
            .unwrap()
            .objects
            .into_iter()
            .map(|o| o.id)
            .collect();
        assert_eq!(ids, [first.id, second.id]);
    }

    #[test]
    fn areas_link_to_entities_and_unlink_when_the_entity_is_deleted() {
        let c = conn();
        let (map, layer) = map_and_layer(&c);
        let faction = make(&c, "faction", "House Varn");
        let mut input = shape(
            &layer,
            MapGeometry::Area {
                points: pts(&[(0.0, 0.0), (10.0, 0.0), (5.0, 8.0)]),
            },
        );
        input.entity_id = Some(faction.clone());
        let area = create_object(&c, input).unwrap();
        assert_eq!(area.entity_name.as_deref(), Some("House Varn"));
        assert_eq!(list_appearances(&c, &faction).unwrap().len(), 1);
        entities::delete(&c, &faction).unwrap();
        let data = get_map(&c, &dir(), &map).unwrap();
        assert_eq!(data.objects.len(), 1);
        assert_eq!(data.objects[0].entity_id, None);
    }
}
