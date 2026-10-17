//! Models for assets (imported images) and maps (background, layers, pins).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use specta::Type;

/// An imported image file. The file lives at `<world>/assets/<id>.<ext>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub id: String,
    /// The original file name, for display only.
    pub file_name: String,
    pub mime: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub created_at: String,
    /// Absolute path of the image file. NOT stored in the database: the
    /// service fills it in from the open world's folder. The frontend turns it
    /// into a URL with `convertFileSrc(path)`.
    pub path: String,
    /// Absolute path of the thumbnail (max 512px PNG), or `None` while it is
    /// still being generated on the background thread (or if that failed).
    pub thumb_path: Option<String>,
}

/// Which kinds of map object exist. Serialised as `{ "kind": "pin", "x": 1, "y": 2 }`.
///
/// This is an "internally tagged" enum: serde writes the variant name into a
/// `kind` field next to the variant's own fields. Phase 10 adds `Path`,
/// `Area` and `Label` variants. The same JSON is stored in
/// `map_objects.geometry`; coordinates are in map pixels (origin top-left).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MapGeometry {
    Pin { x: f64, y: f64 },
}

impl MapGeometry {
    /// The value for the `map_objects.kind` column.
    pub fn kind(&self) -> &'static str {
        match self {
            MapGeometry::Pin { .. } => "pin",
        }
    }
}

/// A map row plus its background asset (if one has been set).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MapInfo {
    /// `maps.id` (internal; commands address maps by `entity_id`).
    pub id: String,
    /// The entity (type `map`) that represents this map in the sidebar.
    pub entity_id: String,
    pub name: String,
    /// Pixels; 0 until a background is set.
    pub width: u32,
    pub height: u32,
    pub background: Option<Asset>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MapLayer {
    pub id: String,
    pub map_id: String,
    pub name: String,
    pub sort_order: i32,
    pub visible: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MapObject {
    pub id: String,
    pub layer_id: String,
    pub geometry: MapGeometry,
    /// Free-form style object (colour, size...). `{}` by default.
    #[specta(type = HashMap<String, specta_typescript::Unknown>)]
    pub style: Value,
    /// The linked entity, or `None` for an unlinked pin.
    pub entity_id: Option<String>,
    /// Convenience copies from the linked entity, so the map can pick an icon
    /// and show a hover label without extra lookups.
    pub entity_type: Option<String>,
    pub entity_name: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Everything the map view needs in one call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MapData {
    pub map: MapInfo,
    pub layers: Vec<MapLayer>,
    pub objects: Vec<MapObject>,
}

/// Input for `create_map_object`. The `kind` column is derived from the
/// geometry variant, so it cannot disagree with it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NewMapObject {
    pub layer_id: String,
    pub geometry: MapGeometry,
    /// Defaults to `{}` when omitted.
    #[specta(type = Option<HashMap<String, specta_typescript::Unknown>>)]
    pub style: Option<Value>,
    pub entity_id: Option<String>,
}

/// Input for `update_map_object`: only the `Some` fields change.
/// `unlink_entity: true` clears the link (a plain `None` entity id means
/// "leave it alone", so a separate flag is needed to remove it).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MapObjectPatch {
    pub geometry: Option<MapGeometry>,
    #[specta(type = Option<HashMap<String, specta_typescript::Unknown>>)]
    pub style: Option<Value>,
    pub entity_id: Option<String>,
    #[serde(default)]
    pub unlink_entity: bool,
}

/// One place where an entity appears as a pin ("appears on maps").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MapAppearance {
    pub map_entity_id: String,
    pub map_name: String,
    pub object_id: String,
    pub geometry: MapGeometry,
}
