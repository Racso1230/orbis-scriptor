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

/// Link kind connecting an entity to an event it took part in.
/// Direction: `source` = the involved entity, `target` = the event
/// (read it as "source is involved in target").
pub const KIND_INVOLVED_IN: &str = "involved_in";

/// Link kind written when an entity-reference field of an entity points at
/// another entity. Direction: `source` = the entity that holds the field,
/// `target` = the referenced entity; `label` = the field label. Kept in sync
/// by `services::properties` on every entity update.
pub const KIND_FIELD_REF: &str = "field_ref";
