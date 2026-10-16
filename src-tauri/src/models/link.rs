use serde::{Deserialize, Serialize};
use specta::Type;

/// Link kind written by the lore editor when a document mentions an entity.
pub const KIND_MENTION: &str = "mention";

/// One entity that mentions another, with a snippet of the surrounding text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Backlink {
    pub source_id: String,
    #[serde(rename = "sourceType")]
    pub source_type: String,
    pub source_name: String,
    pub snippet: String,
}
