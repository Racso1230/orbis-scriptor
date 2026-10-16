use serde::{Deserialize, Serialize};
use specta::Type;

/// The rich-text lore of one entity. One document per entity, created on
/// first save.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub entity_id: String,
    /// TipTap JSON, kept as a string: the backend only stores it, the editor
    /// is the one that understands its shape.
    pub content: String,
    /// Plain text extracted by the editor, used for search (Phase 5).
    pub plain_text: String,
    pub updated_at: String,
}
