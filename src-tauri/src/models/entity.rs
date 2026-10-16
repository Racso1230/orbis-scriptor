use serde::{Deserialize, Serialize};
use serde_json::Value;
use specta::Type;

/// A full entity row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Entity {
    pub id: String,
    /// `type` is a Rust keyword, so the field is `entity_type` in Rust and
    /// renamed to `type` in JSON/TypeScript.
    #[serde(rename = "type")]
    pub entity_type: String,
    pub name: String,
    pub summary: String,
    /// Type-specific fields as a JSON object (see `entity_type::FieldDef`).
    pub properties: Value,
    pub created_at: String,
    pub updated_at: String,
}

/// The light version used by the sidebar list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EntitySummary {
    pub id: String,
    #[serde(rename = "type")]
    pub entity_type: String,
    pub name: String,
    pub updated_at: String,
}

/// Input for `create_entity`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NewEntity {
    #[serde(rename = "type")]
    pub entity_type: String,
    pub name: String,
}

/// Input for `update_entity`: only the fields that are `Some` change.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EntityPatch {
    pub name: Option<String>,
    pub summary: Option<String>,
    pub properties: Option<Value>,
}
