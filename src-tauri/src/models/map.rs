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

/// A point in map pixels (origin top-left). `#[specta(type = Number)]`
/// makes the generated TypeScript say `number` instead of `number | null`
/// (specta otherwise allows `null` because a Rust `f64` could be NaN, which
/// JSON writes as `null`; we reject non-finite values anyway).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Type)]
pub struct Point {
    #[specta(type = specta_typescript::Number)]
    pub x: f64,
    #[specta(type = specta_typescript::Number)]
    pub y: f64,
}

/// Which kinds of map object exist. Serialised as `{ "kind": "pin", "x": 1, "y": 2 }`.
///
/// This is an "internally tagged" enum: serde writes the variant name into a
/// `kind` field next to the variant's own fields. The same JSON is stored in
/// `map_objects.geometry`; coordinates are in map pixels (origin top-left).
/// Rotations are in degrees, clockwise.
///
/// | kind    | `map_objects.kind` | notes |
/// | ------- | ------------------ | ----- |
/// | `pin`   | `pin`   | a point, usually linked to an entity |
/// | `path`  | `path`  | polyline for rivers, roads, borders and freehand strokes (2..=20000 points); `closed` joins the last point to the first. The frontend simplifies/smooths before saving; `style.smoothing` tells the renderer to curve it |
/// | `area`  | `area`  | polygon (3..=20000 points), filled; may link to an entity (kingdom, region) |
/// | `label` | `label` | text (1..=200 chars) at `x`,`y`. If `pathId` is set the text follows that path object (same map); deleting the path detaches the label (`pathId` becomes null, `x`,`y` kept) |
/// | `stamp` | `stamp` | icon from the frontend stamp library (`icon` key: 1..=64 chars of `a-z0-9-`), `scale` > 0 |
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MapGeometry {
    Pin {
        #[specta(type = specta_typescript::Number)]
        x: f64,
        #[specta(type = specta_typescript::Number)]
        y: f64,
    },
    Path {
        points: Vec<Point>,
        closed: bool,
    },
    Area {
        points: Vec<Point>,
    },
    Label {
        #[specta(type = specta_typescript::Number)]
        x: f64,
        #[specta(type = specta_typescript::Number)]
        y: f64,
        text: String,
        #[specta(type = specta_typescript::Number)]
        rotation: f64,
        // `rename` because the enum-level `rename_all` only renames variants.
        #[serde(rename = "pathId", default)]
        path_id: Option<String>,
    },
    Stamp {
        #[specta(type = specta_typescript::Number)]
        x: f64,
        #[specta(type = specta_typescript::Number)]
        y: f64,
        icon: String,
        #[specta(type = specta_typescript::Number)]
        scale: f64,
        #[specta(type = specta_typescript::Number)]
        rotation: f64,
    },
}

impl MapGeometry {
    /// The value for the `map_objects.kind` column.
    pub fn kind(&self) -> &'static str {
        match self {
            MapGeometry::Pin { .. } => "pin",
            MapGeometry::Path { .. } => "path",
            MapGeometry::Area { .. } => "area",
            MapGeometry::Label { .. } => "label",
            MapGeometry::Stamp { .. } => "stamp",
        }
    }
}

/// What a map shows when it has no background image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum BackgroundKind {
    Parchment,
    Ocean,
    Plain,
}

/// The look of a "blank" map (one without a background image). Stored as JSON
/// in `maps.background_style`; `None` there means "image map".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MapBackground {
    pub kind: BackgroundKind,
    /// `#RRGGBB`. The fill colour for `plain`; an optional tint for the
    /// others. `None` = the frontend's default colour for that kind.
    pub color: Option<String>,
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
    /// Look of a blank map; `None` for maps that use (or wait for) an image.
    pub background_style: Option<MapBackground>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MapLayer {
    pub id: String,
    pub map_id: String,
    pub name: String,
    pub sort_order: i32,
    pub visible: bool,
    /// A locked layer rejects creating, changing and deleting its objects.
    pub locked: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MapObject {
    pub id: String,
    pub layer_id: String,
    pub geometry: MapGeometry,
    /// Free-form JSON object (max 4 KB), `{}` by default. Keys the frontend
    /// uses (all optional): `stroke` (#hex), `strokeWidth` (px), `dash`
    /// (number[]), `smoothing` (0..1 curve tension), `fill` (#hex),
    /// `fillOpacity` (0..1), `fillPattern` ("none" | "hatch" | "dots"),
    /// `fontSize`, `fontFamily`, `color` (#hex, text colour).
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
    /// Optional client-chosen UUID. Undo/redo uses it to recreate an object
    /// with the SAME id (so labels, links etc. stay valid). Rejected if an
    /// object with that id already exists. `None` = the server generates one.
    #[serde(default)]
    pub id: Option<String>,
    pub layer_id: String,
    pub geometry: MapGeometry,
    /// Defaults to `{}` when omitted.
    #[specta(type = Option<HashMap<String, specta_typescript::Unknown>>)]
    pub style: Option<Value>,
    pub entity_id: Option<String>,
    /// Optional RFC 3339 creation time. Objects are drawn in creation order
    /// within a layer, so undoing a delete passes the original value to
    /// restore the stacking order.
    #[serde(default)]
    pub created_at: Option<String>,
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
    /// Move the object to another layer of the same map (must not be locked).
    #[serde(default)]
    pub layer_id: Option<String>,
}

/// One step of an atomic batch for `apply_map_ops`. Serialised with an `op`
/// field: `{ "op": "create", "object": {...} }`, `{ "op": "update", "id": ..,
/// "patch": {...} }` or `{ "op": "delete", "id": .. }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum MapOp {
    Create { object: NewMapObject },
    Update { id: String, patch: MapObjectPatch },
    Delete { id: String },
}

/// What `apply_map_ops` changed, in their final state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MapOpsResult {
    /// Objects created or updated by the batch that still exist afterwards.
    pub objects: Vec<MapObject>,
    /// Ids removed by `delete` ops.
    pub deleted_ids: Vec<String>,
    /// Labels that were detached from a deleted path as a side effect
    /// (`pathId` is now null). To undo, restore `pathId` on these.
    pub detached: Vec<MapObject>,
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
