//! The one place that defines the built-in entity types: their keys, labels,
//! icons and type-specific fields. The frontend fetches this list through the
//! `entity_types` command, so adding a type here is enough to make it appear
//! in the sidebar and get a fields form.

use serde::{Deserialize, Serialize};
use specta::Type;

/// How a type-specific field is edited.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum FieldKind {
    Text,
    Number,
    Select,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FieldDef {
    /// Key inside the entity's `properties` JSON object.
    pub key: String,
    pub label: String,
    pub kind: FieldKind,
    /// Choices for `Select`; empty for other kinds.
    pub options: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EntityTypeDef {
    /// Stored in `entities.type`, e.g. `character`.
    pub key: String,
    pub label: String,
    pub plural: String,
    /// A lucide icon name the frontend maps to a component.
    pub icon: String,
    pub fields: Vec<FieldDef>,
}

fn text(key: &str, label: &str) -> FieldDef {
    FieldDef {
        key: key.into(),
        label: label.into(),
        kind: FieldKind::Text,
        options: vec![],
    }
}

fn number(key: &str, label: &str) -> FieldDef {
    FieldDef {
        key: key.into(),
        label: label.into(),
        kind: FieldKind::Number,
        options: vec![],
    }
}

fn select(key: &str, label: &str, options: &[&str]) -> FieldDef {
    FieldDef {
        key: key.into(),
        label: label.into(),
        kind: FieldKind::Select,
        options: options.iter().map(|o| (*o).to_owned()).collect(),
    }
}

fn def(key: &str, label: &str, plural: &str, icon: &str, fields: Vec<FieldDef>) -> EntityTypeDef {
    EntityTypeDef {
        key: key.into(),
        label: label.into(),
        plural: plural.into(),
        icon: icon.into(),
        fields,
    }
}

/// All built-in types, in sidebar order.
pub fn all() -> Vec<EntityTypeDef> {
    vec![
        def(
            "character",
            "Character",
            "Characters",
            "user",
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
            vec![
                select(
                    "kind",
                    "Kind",
                    &["weapon", "armour", "artifact", "tool", "document", "other"],
                ),
                text("owner", "Owner"),
            ],
        ),
        def("note", "Note", "Notes", "sticky-note", vec![]),
        // A map is an entity too; its image and pins live in the `maps*` tables.
        def("map", "Map", "Maps", "map", vec![]),
        // An event is an entity too; its dates live in the `events` table.
        def("event", "Event", "Events", "calendar-days", vec![]),
    ]
}

/// True when `key` names a built-in type.
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
            }
        }
    }
}
