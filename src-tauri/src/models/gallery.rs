use serde::{Deserialize, Serialize};
use specta::Type;

/// An entity's portrait: where the image file is, ready for the frontend's
/// `convertFileSrc(path)`. Use `thumbPath` (when present) for small avatars.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Portrait {
    pub asset_id: String,
    /// Absolute path of the image file.
    pub path: String,
    /// Absolute path of the thumbnail (max 512px), `None` while it is being
    /// generated or if that failed.
    pub thumb_path: Option<String>,
}

/// One gallery image of an entity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EntityImage {
    pub id: String,
    pub entity_id: String,
    pub asset_id: String,
    pub caption: String,
    /// Position in the gallery (0 first).
    pub sort_order: u32,
    /// True for the entity's portrait (at most one image per entity).
    pub is_portrait: bool,
    pub created_at: String,
    /// Original file name of the asset, for display.
    pub file_name: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// Absolute path of the image file (not stored in the database).
    pub path: String,
    /// Absolute path of the thumbnail, `None` while it is being generated.
    pub thumb_path: Option<String>,
}

/// A portrait together with the entity it belongs to (`list_portraits`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EntityPortrait {
    pub entity_id: String,
    pub portrait: Portrait,
}
