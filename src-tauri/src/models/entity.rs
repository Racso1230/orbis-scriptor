use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use specta::Type;

use crate::models::gallery::Portrait;

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
    /// Exported to TypeScript as `{ [key: string]: unknown }` because specta
    /// refuses `serde_json::Value`'s 64-bit numbers.
    #[specta(type = HashMap<String, specta_typescript::Unknown>)]
    pub properties: Value,
    pub created_at: String,
    pub updated_at: String,
    /// Starred by the user.
    pub favorite: bool,
    /// Names of the entity's tags, sorted ignoring case.
    pub tags: Vec<String>,
    /// The portrait image. Only filled in by the `get_entity` command (it
    /// needs the world folder to build file paths); `None` elsewhere.
    pub portrait: Option<Portrait>,
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
    pub favorite: bool,
    /// Names of the entity's tags, sorted ignoring case.
    pub tags: Vec<String>,
    /// The portrait, for avatars. Filled in by the list commands
    /// (`list_entities`, `list_favorites`, `list_recent`,
    /// `list_entities_by_tag`) and `list_portraits`; `None` in summaries
    /// that other commands return (involved entities, graph, ...).
    pub portrait: Option<Portrait>,
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
    #[specta(type = Option<HashMap<String, specta_typescript::Unknown>>)]
    pub properties: Option<Value>,
}
