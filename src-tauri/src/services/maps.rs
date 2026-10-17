//! Map business rules.
//!
//! How a map comes to exist: a map is an *entity* of type `map` (so it shows
//! up in the sidebar, search and links like everything else). There is no
//! separate "create map" command. Instead `services::entities::create` calls
//! [`create_for_entity`] inside its transaction whenever the new entity's type
//! is `map`, which inserts the `maps` row (size 0x0 until a background is set)
//! and one default layer called "Default". All of it commits or rolls back
//! together.

use std::path::Path;

use chrono::{SecondsFormat, Utc};
use rusqlite::Connection;
use serde_json::Value;
use uuid::Uuid;

use crate::db;
use crate::error::{AppError, AppResult};
use crate::models::map::{
    Asset, MapAppearance, MapData, MapGeometry, MapInfo, MapLayer, MapObject, MapObjectPatch,
    NewMapObject,
};
use crate::services::assets;

pub const DEFAULT_LAYER_NAME: &str = "Default";

pub(crate) fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// Creates the `maps` row and the default layer for a freshly inserted
/// `map` entity. Call it inside the same transaction as the entity insert.
pub fn create_for_entity(conn: &Connection, entity_id: &str) -> AppResult<()> {
    let map_id = Uuid::now_v7().to_string();
    db::maps::insert_map(conn, &map_id, entity_id)?;
    db::maps::insert_layer(
        conn,
        &MapLayer {
            id: Uuid::now_v7().to_string(),
            map_id,
            name: DEFAULT_LAYER_NAME.to_owned(),
            sort_order: 0,
            visible: true,
        },
    )?;
    Ok(())
}

// ---- reading ------------------------------------------------------------

/// Adds the on-disk paths to the background asset, if there is one.
fn fill_paths(mut info: MapInfo, assets_dir: &Path) -> MapInfo {
    info.background = info.background.map(|a| assets::with_paths(a, assets_dir));
    info
}

/// Everything the map view needs, looked up by the map's *entity* id.
pub fn get_map(conn: &Connection, assets_dir: &Path, entity_id: &str) -> AppResult<MapData> {
    let info = db::maps::get_map_by_entity(conn, entity_id)?
        .ok_or_else(|| AppError::NotFound(format!("No map for entity {entity_id}")))?;
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
pub fn set_background(
    conn: &Connection,
    assets_dir: &Path,
    entity_id: &str,
    asset_id: &str,
) -> AppResult<MapInfo> {
    let info = db::maps::get_map_by_entity(conn, entity_id)?
        .ok_or_else(|| AppError::NotFound(format!("No map for entity {entity_id}")))?;
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

// ---- objects ------------------------------------------------------------

/// Rejects NaN/infinite coordinates (JSON cannot carry them, but be safe).
fn validate_geometry(geometry: &MapGeometry) -> AppResult<()> {
    match geometry {
        MapGeometry::Pin { x, y } => {
            if !x.is_finite() || !y.is_finite() {
                return Err(AppError::Validation("Pin position must be finite".into()));
            }
        }
    }
    Ok(())
}

fn validate_style(style: &Value) -> AppResult<()> {
    if style.is_object() {
        Ok(())
    } else {
        Err(AppError::Validation("Style must be a JSON object".into()))
    }
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

fn reload(conn: &Connection, id: &str) -> AppResult<MapObject> {
    db::maps::get_object(conn, id)?
        .ok_or_else(|| AppError::NotFound(format!("Map object {id} does not exist")))
}

pub fn create_object(conn: &Connection, input: NewMapObject) -> AppResult<MapObject> {
    validate_geometry(&input.geometry)?;
    let style = input
        .style
        .unwrap_or_else(|| Value::Object(Default::default()));
    validate_style(&style)?;
    if !db::maps::layer_exists(conn, &input.layer_id)? {
        return Err(AppError::NotFound(format!(
            "Layer {} does not exist",
            input.layer_id
        )));
    }
    if let Some(entity_id) = &input.entity_id {
        validate_entity(conn, entity_id)?;
    }
    let timestamp = now();
    let id = Uuid::now_v7().to_string();
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
            created_at: timestamp.clone(),
            updated_at: timestamp,
        },
    )?;
    // Reload so the entity type/name from the join are filled in.
    reload(conn, &id)
}

pub fn update_object(conn: &Connection, id: &str, patch: MapObjectPatch) -> AppResult<MapObject> {
    let mut object = reload(conn, id)?;
    if let Some(geometry) = patch.geometry {
        validate_geometry(&geometry)?;
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
    object.updated_at = now();
    db::maps::update_object(conn, &object)?;
    reload(conn, id)
}

pub fn delete_object(conn: &Connection, id: &str) -> AppResult<()> {
    db::maps::delete_object(conn, id)
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
            layer_id: layer_id.into(),
            geometry: MapGeometry::Pin { x, y },
            style: None,
            entity_id,
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
        assert!(serde_json::from_str::<MapGeometry>(r#"{"kind":"path"}"#).is_err());

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
}
