//! Checking an entity's `properties` against its type's field definitions,
//! and keeping the `field_ref` links in step with entity-reference fields.
//!
//! Rules:
//! - Only values that CHANGED are validated. An old value that no longer fits
//!   (a removed select option, a field whose kind changed) never blocks
//!   saving the rest of the form.
//! - Keys that match no field are kept untouched and never validated: when a
//!   field is removed from a type its values stay in the data.
//! - `null`, `""` and `[]` all mean "not set" and are always accepted.
//! - Entity references store entity ids. An id whose entity was deleted is
//!   allowed to stay (the UI shows it as missing).

use rusqlite::Connection;
use serde_json::Value;

use crate::db;
use crate::error::{AppError, AppResult};
use crate::models::entity_type::{FieldDef, FieldKind};
use crate::models::link::KIND_FIELD_REF;
use crate::services::calendar;

const MAX_TEXT_CHARS: usize = 2_000;
const MAX_LONG_TEXT_CHARS: usize = 50_000;
const MAX_REFS: usize = 200;

fn bad(field: &FieldDef, why: &str) -> AppError {
    AppError::Validation(format!("{}: {why}", field.label))
}

/// True for the "not set" values.
fn is_empty(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(s) => s.is_empty(),
        Value::Array(a) => a.is_empty(),
        _ => false,
    }
}

/// Validates every field whose value differs between `old` and `new`.
pub fn validate_changes(
    conn: &Connection,
    fields: &[FieldDef],
    old: &Value,
    new: &Value,
) -> AppResult<()> {
    for field in fields {
        let after = new.get(&field.key);
        // `Option<&Value>` compares by value, so "absent" vs "present" counts as a change.
        if after == old.get(&field.key) {
            continue;
        }
        if let Some(value) = after
            && !is_empty(value)
        {
            validate_value(conn, field, value)?;
        }
    }
    Ok(())
}

fn text_of<'a>(field: &FieldDef, value: &'a Value) -> AppResult<&'a str> {
    value.as_str().ok_or_else(|| bad(field, "expected text"))
}

fn strings_of<'a>(field: &FieldDef, value: &'a Value) -> AppResult<Vec<&'a str>> {
    let items = value
        .as_array()
        .ok_or_else(|| bad(field, "expected a list"))?;
    items
        .iter()
        .map(|v| {
            v.as_str()
                .ok_or_else(|| bad(field, "expected a list of text"))
        })
        .collect()
}

fn validate_value(conn: &Connection, field: &FieldDef, value: &Value) -> AppResult<()> {
    match &field.kind {
        FieldKind::Text => {
            if text_of(field, value)?.chars().count() > MAX_TEXT_CHARS {
                return Err(bad(field, "text is too long"));
            }
        }
        FieldKind::LongText => {
            if text_of(field, value)?.chars().count() > MAX_LONG_TEXT_CHARS {
                return Err(bad(field, "text is too long"));
            }
        }
        FieldKind::Url => {
            let url = text_of(field, value)?;
            let ok = ["http://", "https://", "mailto:"]
                .iter()
                .any(|p| url.starts_with(p));
            if !ok || url.chars().count() > MAX_TEXT_CHARS {
                return Err(bad(
                    field,
                    "enter a link starting with http://, https:// or mailto:",
                ));
            }
        }
        FieldKind::Number => {
            if !value.is_number() {
                return Err(bad(field, "expected a number"));
            }
        }
        FieldKind::Boolean => {
            if !value.is_boolean() {
                return Err(bad(field, "expected true or false"));
            }
        }
        FieldKind::Select => {
            let choice = text_of(field, value)?;
            if !field.options.iter().any(|o| o == choice) {
                return Err(bad(
                    field,
                    &format!("\"{choice}\" is not one of the options"),
                ));
            }
        }
        FieldKind::MultiSelect => {
            let choices = strings_of(field, value)?;
            for (i, choice) in choices.iter().enumerate() {
                if !field.options.iter().any(|o| o == choice) {
                    return Err(bad(
                        field,
                        &format!("\"{choice}\" is not one of the options"),
                    ));
                }
                if choices[..i].contains(choice) {
                    return Err(bad(field, &format!("\"{choice}\" is listed twice")));
                }
            }
        }
        FieldKind::Date => {
            // A whole number of days from the world's epoch (may be negative).
            let day = value
                .as_i64()
                .ok_or_else(|| bad(field, "expected a day number"))?;
            calendar::check_day(day)?;
        }
        FieldKind::EntityRef {
            allowed_types,
            multiple,
        } => {
            let ids: Vec<&str> = if *multiple {
                strings_of(field, value)?
            } else {
                vec![text_of(field, value)?]
            };
            if ids.len() > MAX_REFS {
                return Err(bad(field, "too many references"));
            }
            for (i, id) in ids.iter().enumerate() {
                if ids[..i].contains(id) {
                    return Err(bad(field, "the same entity is listed twice"));
                }
                // Unknown ids are allowed (dangling reference); known ones
                // must have an allowed type.
                if !allowed_types.is_empty()
                    && let Some(target) = db::entities::get(conn, id)?
                    && !allowed_types.contains(&target.entity_type)
                {
                    return Err(bad(
                        field,
                        &format!(
                            "\"{}\" is a {}, which is not allowed here",
                            target.name, target.entity_type
                        ),
                    ));
                }
            }
        }
    }
    Ok(())
}

/// The `(target id, field label)` pairs for every entity-reference value in
/// `properties`, in field order, without duplicates and without
/// self-references.
pub fn field_refs(
    entity_id: &str,
    fields: &[FieldDef],
    properties: &Value,
) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for field in fields {
        if !matches!(field.kind, FieldKind::EntityRef { .. }) {
            continue;
        }
        let ids: Vec<&str> = match properties.get(&field.key) {
            Some(Value::String(s)) => vec![s.as_str()],
            Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).collect(),
            _ => Vec::new(),
        };
        for id in ids {
            if !id.is_empty() && id != entity_id && !out.iter().any(|(t, _)| t == id) {
                out.push((id.to_owned(), field.label.clone()));
            }
        }
    }
    out
}

/// Makes the `field_ref` links of `entity_id` match its reference fields.
///
/// - `source` = the entity holding the field, `target` = the referenced
///   entity, `label` = the field's label.
/// - If two fields point at the same target only one link exists (the first
///   field's label), because `links` is unique per (source, target, kind).
/// - Targets that no longer exist are skipped.
/// - Only rows of kind `field_ref` are touched, never mentions,
///   relationships or `involved_in` links.
pub fn sync_field_refs(
    conn: &Connection,
    entity_id: &str,
    fields: &[FieldDef],
    properties: &Value,
) -> AppResult<()> {
    let refs = field_refs(entity_id, fields, properties);
    db::links::replace_from_source_labeled(conn, entity_id, KIND_FIELD_REF, &refs)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::db::migrations::MIGRATIONS;
    use crate::models::entity::{EntityPatch, NewEntity};
    use crate::models::entity_type::FieldInput;
    use crate::models::link::KIND_MENTION;
    use crate::services::{documents, entities, entity_types, relationships};
    use serde_json::json;

    fn conn() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        c.pragma_update(None, "foreign_keys", "ON").unwrap();
        MIGRATIONS.to_latest(&mut c).unwrap();
        c
    }

    fn field(label: &str, kind: FieldKind) -> FieldInput {
        FieldInput {
            key: None,
            label: label.into(),
            kind,
            options: vec![],
        }
    }

    /// A custom "Thing" type with one field of every kind.
    fn thing_type(c: &Connection) -> String {
        let sel = |label: &str, kind: FieldKind| FieldInput {
            options: vec!["a".into(), "b".into()],
            ..field(label, kind)
        };
        entity_types::create(
            c,
            "Thing",
            "Things",
            "box",
            "#112233",
            vec![
                field("Text", FieldKind::Text),
                field("Long", FieldKind::LongText),
                field("Num", FieldKind::Number),
                field("Flag", FieldKind::Boolean),
                sel("Pick", FieldKind::Select),
                sel("Picks", FieldKind::MultiSelect),
                field("When", FieldKind::Date),
                field("Site", FieldKind::Url),
                field(
                    "Boss",
                    FieldKind::EntityRef {
                        allowed_types: vec!["character".into()],
                        multiple: false,
                    },
                ),
                field(
                    "Allies",
                    FieldKind::EntityRef {
                        allowed_types: vec![],
                        multiple: true,
                    },
                ),
            ],
        )
        .unwrap()
        .key
    }

    fn new_entity(c: &Connection, t: &str, name: &str) -> String {
        entities::create(
            c,
            NewEntity {
                entity_type: t.into(),
                name: name.into(),
            },
        )
        .unwrap()
        .id
    }

    fn set(c: &Connection, id: &str, props: Value) -> AppResult<()> {
        entities::update(
            c,
            id,
            EntityPatch {
                properties: Some(props),
                ..Default::default()
            },
        )
        .map(|_| ())
    }

    #[test]
    fn accepts_good_values_of_every_kind() {
        let c = conn();
        let t = thing_type(&c);
        let id = new_entity(&c, &t, "Gizmo");
        let boss = new_entity(&c, "character", "Ada");
        let other = new_entity(&c, "location", "Town");
        set(
            &c,
            &id,
            json!({
                "text": "hi", "long": "a\nb", "num": 3.5, "flag": false, "pick": "a",
                "picks": ["a", "b"], "when": -400, "site": "https://example.com",
                "boss": boss, "allies": [boss, other, "dangling-id"]
            }),
        )
        .unwrap();
        // Unset values are always fine.
        set(
            &c,
            &id,
            json!({ "text": "", "num": null, "picks": [], "boss": "" }),
        )
        .unwrap();
    }

    #[test]
    fn rejects_bad_values_per_kind() {
        let c = conn();
        let t = thing_type(&c);
        let id = new_entity(&c, &t, "Gizmo");
        let town = new_entity(&c, "location", "Town");
        let ada = new_entity(&c, "character", "Ada");
        let bad_values = [
            json!({ "text": 5 }),
            json!({ "long": ["x"] }),
            json!({ "num": "7" }),
            json!({ "flag": "yes" }),
            json!({ "pick": "c" }),
            json!({ "pick": 1 }),
            json!({ "picks": ["a", "c"] }),
            json!({ "picks": ["a", "a"] }),
            json!({ "picks": "a" }),
            json!({ "when": 1.5 }),
            json!({ "when": "5" }),
            json!({ "when": 1_000_000_000_000_000_i64 }),
            json!({ "site": "javascript:alert(1)" }),
            json!({ "site": "example.com" }),
            json!({ "boss": [ada] }),
            json!({ "boss": town }), // wrong type (location)
            json!({ "allies": ada }),
            json!({ "allies": [ada, ada] }),
            json!({ "allies": [5] }),
            json!({ "text": "x".repeat(2001) }),
        ];
        for props in bad_values {
            assert!(
                matches!(set(&c, &id, props.clone()), Err(AppError::Validation(_))),
                "{props}"
            );
        }
        // Nothing was saved by the failed updates.
        assert_eq!(entities::get(&c, &id).unwrap().properties, json!({}));
    }

    #[test]
    fn unknown_keys_are_kept_and_unchanged_values_are_not_revalidated() {
        let c = conn();
        let t = thing_type(&c);
        let id = new_entity(&c, &t, "Gizmo");
        set(
            &c,
            &id,
            json!({ "pick": "a", "legacy": { "anything": [1] } }),
        )
        .unwrap();
        assert_eq!(
            entities::get(&c, &id).unwrap().properties["legacy"],
            json!({ "anything": [1] })
        );
        // Remove the option "a" from the field: the stored value is now
        // invalid, but saving other fields still works.
        let upd = |label: &str, key: &str, options: Vec<String>| FieldInput {
            key: Some(key.into()),
            options,
            ..field(label, FieldKind::Select)
        };
        entity_types::update(
            &c,
            &t,
            None,
            None,
            None,
            None,
            Some(vec![
                upd("Pick", "pick", vec!["z".into()]),
                FieldInput {
                    key: Some("text".into()),
                    ..field("Text", FieldKind::Text)
                },
            ]),
        )
        .unwrap();
        set(&c, &id, json!({ "pick": "a", "text": "ok", "legacy": 1 })).unwrap();
        // Changing it to another invalid value is refused.
        assert!(set(&c, &id, json!({ "pick": "b", "text": "ok" })).is_err());
        // Fields that no longer exist are not validated at all.
        set(&c, &id, json!({ "pick": "a", "num": "not a number" })).unwrap();
    }

    #[test]
    fn changing_a_fields_kind_keeps_old_values() {
        let c = conn();
        let t = thing_type(&c);
        let id = new_entity(&c, &t, "Gizmo");
        set(&c, &id, json!({ "text": "hello" })).unwrap();
        let as_number = FieldInput {
            key: Some("text".into()),
            ..field("Text", FieldKind::Number)
        };
        entity_types::update(&c, &t, None, None, None, None, Some(vec![as_number])).unwrap();
        assert_eq!(entities::get(&c, &id).unwrap().properties["text"], "hello");
    }

    #[test]
    fn field_ref_links_follow_the_values() {
        let c = conn();
        let t = thing_type(&c);
        let id = new_entity(&c, &t, "Gizmo");
        let ada = new_entity(&c, "character", "Ada");
        let bob = new_entity(&c, "character", "Bob");
        let links = |c: &Connection, src: &str| {
            db::links::targets_for_source(c, src, KIND_FIELD_REF).unwrap()
        };

        set(
            &c,
            &id,
            json!({ "boss": ada, "allies": [bob, ada, "gone"] }),
        )
        .unwrap();
        let mut expect = vec![ada.clone(), bob.clone()];
        expect.sort();
        assert_eq!(links(&c, &id), expect);
        // The first field to mention a target names the link.
        let row = db::links::find_link(&c, &id, &ada, KIND_FIELD_REF)
            .unwrap()
            .unwrap();
        assert_eq!(row.label.as_deref(), Some("Boss"));
        let row = db::links::find_link(&c, &id, &bob, KIND_FIELD_REF)
            .unwrap()
            .unwrap();
        assert_eq!(row.label.as_deref(), Some("Allies"));

        // Removing a reference removes its link.
        set(&c, &id, json!({ "allies": [bob] })).unwrap();
        assert_eq!(links(&c, &id), vec![bob.clone()]);
        // Self-references create no link.
        set(&c, &id, json!({ "allies": [id] })).unwrap();
        assert!(links(&c, &id).is_empty());

        // Deleting the target removes the link but leaves the stored id.
        set(&c, &id, json!({ "boss": ada })).unwrap();
        entities::delete(&c, &ada).unwrap();
        assert!(links(&c, &id).is_empty());
        assert_eq!(
            entities::get(&c, &id).unwrap().properties["boss"],
            json!(ada)
        );
    }

    #[test]
    fn field_ref_sync_does_not_touch_mentions_or_relationships() {
        let c = conn();
        let t = thing_type(&c);
        let id = new_entity(&c, &t, "Gizmo");
        let ada = new_entity(&c, "character", "Ada");
        let bob = new_entity(&c, "character", "Bob");
        documents::save(
            &c,
            &id,
            json!({ "type": "doc", "content": [{ "type": "paragraph", "content": [
                { "type": "mention", "attrs": { "id": ada } }
            ]}]})
            .to_string(),
            "Ada".into(),
        )
        .unwrap();
        relationships::create(&c, &ada, &bob, "friend_of", None).unwrap();
        db::links::insert_link(&c, &id, &ada, "involved_in").unwrap();

        set(&c, &id, json!({ "boss": ada })).unwrap();
        set(&c, &id, json!({})).unwrap();
        let count = |kind: &str| -> i64 {
            c.query_row("SELECT count(*) FROM links WHERE kind = ?1", [kind], |r| {
                r.get(0)
            })
            .unwrap()
        };
        assert_eq!(count(KIND_MENTION), 1);
        assert_eq!(count("friend_of"), 1);
        assert_eq!(count("involved_in"), 1);
        assert_eq!(count(KIND_FIELD_REF), 0);

        // And saving the document does not remove field_ref links.
        set(&c, &id, json!({ "boss": ada })).unwrap();
        documents::save(
            &c,
            &id,
            r#"{"type":"doc","content":[]}"#.into(),
            String::new(),
        )
        .unwrap();
        assert_eq!(count(KIND_FIELD_REF), 1);
        assert_eq!(count(KIND_MENTION), 0);
    }

    #[test]
    fn changing_the_type_definition_resyncs_links() {
        let c = conn();
        let t = thing_type(&c);
        let id = new_entity(&c, &t, "Gizmo");
        let ada = new_entity(&c, "character", "Ada");
        set(&c, &id, json!({ "boss": ada })).unwrap();
        assert_eq!(
            db::links::targets_for_source(&c, &id, KIND_FIELD_REF)
                .unwrap()
                .len(),
            1
        );
        // Remove every field: the value stays in the data but the link goes.
        entity_types::update(&c, &t, None, None, None, None, Some(vec![])).unwrap();
        assert!(
            db::links::targets_for_source(&c, &id, KIND_FIELD_REF)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            entities::get(&c, &id).unwrap().properties["boss"],
            json!(ada)
        );
    }

    #[test]
    fn built_in_types_can_have_reference_fields() {
        let c = conn();
        entity_types::set_template_fields(
            &c,
            "item",
            vec![field(
                "Forged by",
                FieldKind::EntityRef {
                    allowed_types: vec!["character".into()],
                    multiple: false,
                },
            )],
        )
        .unwrap();
        let sword = new_entity(&c, "item", "Sword");
        let smith = new_entity(&c, "character", "Smith");
        set(&c, &sword, json!({ "kind": "weapon", "forged_by": smith })).unwrap();
        assert_eq!(
            db::links::targets_for_source(&c, &sword, KIND_FIELD_REF).unwrap(),
            vec![smith]
        );
    }

    #[test]
    fn graph_shows_field_refs_only_when_asked() {
        use crate::models::relationship::GraphFilter;
        use crate::services::graph;
        let c = conn();
        let t = thing_type(&c);
        let id = new_entity(&c, &t, "Gizmo");
        let ada = new_entity(&c, "character", "Ada");
        set(&c, &id, json!({ "boss": ada })).unwrap();

        let default = graph::get_entity_graph(&c, &id, 1, &GraphFilter::default()).unwrap();
        assert_eq!(default.nodes.len(), 1);
        let filter = GraphFilter {
            kinds: Some(vec![KIND_FIELD_REF.into()]),
            // Custom types are valid in the type filter.
            entity_types: Some(vec![t.clone(), "character".into()]),
            ..Default::default()
        };
        let g = graph::get_entity_graph(&c, &id, 1, &filter).unwrap();
        assert_eq!(g.nodes.len(), 2);
        assert_eq!(g.edges.len(), 1);
        assert_eq!(
            (g.edges[0].kind.as_str(), g.edges[0].label.as_str()),
            (KIND_FIELD_REF, "Boss")
        );
        // Not a relationship: cannot be created or edited by hand.
        assert!(relationships::create(&c, &id, &ada, KIND_FIELD_REF, None).is_err());
    }
}
