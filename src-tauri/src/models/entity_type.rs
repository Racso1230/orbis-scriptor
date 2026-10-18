//! Entity types and their fields.
//!
//! Two sources feed the type list the frontend sees (merged by
//! `services::entity_types`):
//! - the built-in catalog below (fixed keys, labels and fields), and
//! - types the user defines per world (`custom_entity_types` table), plus
//!   extra user fields on any type (`type_templates` table).
//!
//! Map and event behaviour (their extra tables) stays tied to the built-in
//! keys `map` and `event`.

use serde::{Deserialize, Serialize};
use specta::Type;

/// How a type-specific field is edited and what its value in an entity's
/// `properties` JSON looks like.
///
/// Serde writes unit variants as plain strings (`"text"`) and the data
/// variant as `{ "entityRef": { "allowedTypes": [...], "multiple": true } }`.
///
/// | kind          | JSON value in `properties[key]` |
/// | ------------- | ------------------------------- |
/// | `text`        | string |
/// | `longText`    | string (multi-line) |
/// | `number`      | number |
/// | `boolean`     | `true` / `false` |
/// | `select`      | string, one of `options` |
/// | `multiSelect` | array of strings, each one of `options` |
/// | `date`        | integer in-world day number (format with the display calendar) |
/// | `entityRef`   | entity id string (`multiple: false`) or array of ids (`multiple: true`) |
/// | `url`         | string starting with `http://`, `https://` or `mailto:` |
///
/// A missing key, `null` or an empty string/array all mean "not set".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum FieldKind {
    Text,
    Number,
    Select,
    LongText,
    Boolean,
    Date,
    /// Reference(s) to other entities. `allowedTypes` empty = any type.
    EntityRef {
        // `rename` because the enum-level `rename_all` only renames variants.
        #[serde(rename = "allowedTypes")]
        allowed_types: Vec<String>,
        multiple: bool,
    },
    MultiSelect,
    Url,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FieldDef {
    /// Key inside the entity's `properties` JSON object.
    pub key: String,
    pub label: String,
    pub kind: FieldKind,
    /// Choices for `Select` / `MultiSelect`; empty for other kinds.
    pub options: Vec<String>,
    /// True for fields the user defined (editable); false for the built-in
    /// fields of the built-in types.
    pub custom: bool,
}

/// A field as sent by the frontend when creating or editing a type.
/// `key` is `None` for a new field (the backend generates it from the label);
/// an existing field must send its current key back so stored values stay
/// attached to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FieldInput {
    pub key: Option<String>,
    pub label: String,
    pub kind: FieldKind,
    pub options: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EntityTypeDef {
    /// Stored in `entities.type`, e.g. `character` or `custom-deity`.
    pub key: String,
    pub label: String,
    pub plural: String,
    /// A lucide icon name the frontend maps to a component
    /// (see `services::entity_types::type_icons`).
    pub icon: String,
    /// `#rrggbb`, used for map pins and graph nodes.
    pub color: String,
    /// True for the built-in types (cannot be renamed or deleted).
    pub builtin: bool,
    pub fields: Vec<FieldDef>,
}

fn text(key: &str, label: &str) -> FieldDef {
    FieldDef {
        key: key.into(),
        label: label.into(),
        kind: FieldKind::Text,
        options: vec![],
        custom: false,
    }
}

fn number(key: &str, label: &str) -> FieldDef {
    FieldDef {
        key: key.into(),
        label: label.into(),
        kind: FieldKind::Number,
        options: vec![],
        custom: false,
    }
}

fn select(key: &str, label: &str, options: &[&str]) -> FieldDef {
    FieldDef {
        key: key.into(),
        label: label.into(),
        kind: FieldKind::Select,
        options: options.iter().map(|o| (*o).to_owned()).collect(),
        custom: false,
    }
}

fn def(
    key: &str,
    label: &str,
    plural: &str,
    icon: &str,
    color: &str,
    fields: Vec<FieldDef>,
) -> EntityTypeDef {
    EntityTypeDef {
        key: key.into(),
        label: label.into(),
        plural: plural.into(),
        icon: icon.into(),
        color: color.into(),
        builtin: true,
        fields,
    }
}

/// Colour given to custom types that do not pick one (matches the pin look
/// the frontend used for unknown types).
pub const DEFAULT_CUSTOM_COLOR: &str = "#0891b2";

/// All built-in types, in sidebar order.
pub fn all() -> Vec<EntityTypeDef> {
    vec![
        def(
            "character",
            "Character",
            "Characters",
            "user",
            "#2563eb",
            vec![
                text("species", "Species"),
                number("age", "Age"),
                select("status", "Status", &["alive", "dead", "unknown"]),
            ],
        ),
        def(
            "location",
            "Location",
            "Locations",
            "map-pin",
            "#16a34a",
            vec![
                select(
                    "kind",
                    "Kind",
                    &["city", "town", "village", "region", "landmark", "other"],
                ),
                text("region", "Region"),
            ],
        ),
        def(
            "faction",
            "Faction",
            "Factions",
            "flag",
            "#dc2626",
            vec![
                select(
                    "kind",
                    "Kind",
                    &["kingdom", "guild", "religion", "house", "company", "other"],
                ),
                text("leader", "Leader"),
            ],
        ),
        def(
            "item",
            "Item",
            "Items",
            "sword",
            "#d97706",
            vec![
                select(
                    "kind",
                    "Kind",
                    &["weapon", "armour", "artifact", "tool", "document", "other"],
                ),
                text("owner", "Owner"),
            ],
        ),
        def("note", "Note", "Notes", "sticky-note", "#ca8a04", vec![]),
        // A map is an entity too; its image and pins live in the `maps*` tables.
        def("map", "Map", "Maps", "map", "#7c3aed", vec![]),
        // An event is an entity too; its dates live in the `events` table.
        def(
            "event",
            "Event",
            "Events",
            "calendar-days",
            "#db2777",
            vec![],
        ),
    ]
}

/// True when `key` names a BUILT-IN type. Use
/// `services::entity_types::is_known` to include custom types.
pub fn is_valid(key: &str) -> bool {
    all().iter().any(|t| t.key == key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_unique_and_validated() {
        let types = all();
        let mut keys: Vec<&str> = types.iter().map(|t| t.key.as_str()).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), types.len());
        assert!(is_valid("character"));
        assert!(is_valid("map"));
        assert!(is_valid("event"));
        assert!(!is_valid("dragon"));
    }

    #[test]
    fn select_fields_have_options() {
        for t in all() {
            for f in t.fields {
                assert_eq!(
                    f.kind == FieldKind::Select,
                    !f.options.is_empty(),
                    "{}",
                    f.key
                );
                assert!(!f.custom);
            }
        }
    }
}
