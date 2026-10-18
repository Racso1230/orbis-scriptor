//! Typed relationships between entities, stored as rows in `links`.
//!
//! Rules (see also `models::relationship` for the catalog):
//! - `mention` (synced from lore) and `involved_in` (events) rows live in the
//!   same table but are NOT relationships: nothing here may create, read for
//!   editing, change or delete them.
//! - A relationship is exactly one row `(source_id, target_id, kind, label)`.
//! - Symmetric kinds (spouse, sibling, ally...) are stored once with the
//!   smaller id as `source_id`, so A-B and B-A are the same row and a
//!   duplicate in either direction hits the existing row.
//! - `custom` relationships carry free text in `label`. The table's UNIQUE
//!   (source, target, kind) allows only ONE custom link per ordered pair; to
//!   say two things about the same pair, put them in one label. A->B and
//!   B->A can each have their own custom link.
//! - No self-relationships.

use rusqlite::Connection;
use uuid::Uuid;

use crate::db;
use crate::db::links::LinkRow;
use crate::error::{AppError, AppResult};
use crate::models::link::{KIND_FIELD_REF, KIND_INVOLVED_IN, KIND_MENTION};
use crate::models::relationship::{
    self as catalog, KIND_CUSTOM, RelationshipCategory, RelationshipKindDef, RelationshipRecord,
    RelationshipView,
};
use crate::services::entities;

const MAX_LABEL_CHARS: usize = 100;

/// The catalog the frontend renders pickers and labels from.
pub fn list_kinds() -> Vec<RelationshipKindDef> {
    catalog::all()
}

/// Keys of every relationship kind (excludes `mention`, `involved_in` and `field_ref`).
pub fn relationship_kind_keys() -> Vec<String> {
    catalog::all().into_iter().map(|k| k.key).collect()
}

/// True for the link kinds owned by other features (lore mentions, event
/// participation and entity-reference fields).
pub fn is_reserved_kind(kind: &str) -> bool {
    kind == KIND_MENTION || kind == KIND_INVOLVED_IN || kind == KIND_FIELD_REF
}

/// Looks up a kind, with clear errors for unknown and reserved ones.
fn kind_def(kind: &str) -> AppResult<RelationshipKindDef> {
    if is_reserved_kind(kind) {
        return Err(AppError::Validation(format!(
            "\"{kind}\" links are managed automatically and are not relationships"
        )));
    }
    catalog::find(kind)
        .ok_or_else(|| AppError::Validation(format!("Unknown relationship kind \"{kind}\"")))
}

/// Checks one end of the link against a list of allowed types
/// (an empty list means anything goes).
fn check_end(
    def: &RelationshipKindDef,
    role: &str,
    allowed: &[String],
    actual: &str,
) -> AppResult<()> {
    if allowed.is_empty() || allowed.iter().any(|t| t == actual) {
        return Ok(());
    }
    Err(AppError::Validation(format!(
        "\"{}\" needs a {} of type {} (got {actual})",
        def.forward_label,
        role,
        allowed.join(" or ")
    )))
}

/// Custom links need text; every other kind drops any label it is given.
fn clean_label(def: &RelationshipKindDef, label: Option<String>) -> AppResult<Option<String>> {
    if !def.needs_label {
        return Ok(None);
    }
    let text = label.unwrap_or_default().trim().to_owned();
    if text.is_empty() {
        return Err(AppError::Validation(
            "A custom relationship needs a label".into(),
        ));
    }
    if text.chars().count() > MAX_LABEL_CHARS {
        return Err(AppError::Validation(format!(
            "A relationship label can have at most {MAX_LABEL_CHARS} characters"
        )));
    }
    Ok(Some(text))
}

/// Validates a would-be relationship and returns it in storage form
/// (canonical order for symmetric kinds). `existing_id` is the row being
/// edited, so it does not count as its own duplicate.
fn prepare(
    conn: &Connection,
    def: &RelationshipKindDef,
    source_id: &str,
    target_id: &str,
    label: Option<String>,
    existing_id: Option<&str>,
) -> AppResult<LinkRow> {
    if source_id == target_id {
        return Err(AppError::Validation(
            "An entity cannot have a relationship with itself".into(),
        ));
    }
    let source = entities::get(conn, source_id)?;
    let target = entities::get(conn, target_id)?;
    check_end(def, "source", &def.source_types, &source.entity_type)?;
    check_end(def, "target", &def.target_types, &target.entity_type)?;
    let label = clean_label(def, label)?;

    // Symmetric kinds: always store the smaller id first.
    let (s, t) = if def.symmetric && target_id < source_id {
        (target_id, source_id)
    } else {
        (source_id, target_id)
    };
    if let Some(found) = db::links::find_link(conn, s, t, &def.key)?
        && Some(found.id.as_str()) != existing_id
    {
        return Err(AppError::Validation(format!(
            "{} is already {} {}",
            source.name,
            // The wording reads naturally for both directions of a symmetric kind.
            def.forward_label,
            target.name
        )));
    }
    Ok(LinkRow {
        id: existing_id.map_or_else(|| Uuid::now_v7().to_string(), str::to_owned),
        source_id: s.to_owned(),
        target_id: t.to_owned(),
        kind: def.key.clone(),
        label,
    })
}

fn record(row: &LinkRow) -> RelationshipRecord {
    RelationshipRecord {
        id: row.id.clone(),
        source_id: row.source_id.clone(),
        target_id: row.target_id.clone(),
        kind: row.kind.clone(),
        label: row.label.clone(),
    }
}

/// Creates "`source` <kind> `target`" (e.g. source is *parent of* target).
pub fn create(
    conn: &Connection,
    source_id: &str,
    target_id: &str,
    kind: &str,
    label: Option<String>,
) -> AppResult<RelationshipRecord> {
    let def = kind_def(kind)?;
    let row = prepare(conn, &def, source_id, target_id, label, None)?;
    db::links::insert_row(conn, &row)?;
    Ok(record(&row))
}

/// Loads a link by id and makes sure it is a real relationship.
fn load_relationship(conn: &Connection, link_id: &str) -> AppResult<LinkRow> {
    let row = db::links::get_link(conn, link_id)?
        .ok_or_else(|| AppError::NotFound(format!("Relationship {link_id} does not exist")))?;
    if is_reserved_kind(&row.kind) {
        return Err(AppError::Validation(
            "That link is not a relationship".into(),
        ));
    }
    Ok(row)
}

/// Changes a relationship's kind, label and/or direction.
/// `None` leaves a field as it is. Changing to a non-custom kind clears the
/// label; the result is validated like a new relationship.
pub fn update(
    conn: &Connection,
    link_id: &str,
    kind: Option<String>,
    label: Option<String>,
    swap_direction: Option<bool>,
) -> AppResult<RelationshipRecord> {
    let current = load_relationship(conn, link_id)?;
    let def = kind_def(kind.as_deref().unwrap_or(&current.kind))?;
    let (mut source, mut target) = (current.source_id.clone(), current.target_id.clone());
    if swap_direction == Some(true) {
        std::mem::swap(&mut source, &mut target);
    }
    // Keep the old text only when the kind stays custom and no new text came.
    let label = label.or_else(|| current.label.clone());
    let row = prepare(conn, &def, &source, &target, label, Some(link_id))?;
    db::links::update_row(conn, &row)?;
    Ok(record(&row))
}

pub fn delete(conn: &Connection, link_id: &str) -> AppResult<()> {
    load_relationship(conn, link_id)?;
    db::links::delete_by_id(conn, link_id)?;
    Ok(())
}

/// What a link says, read from one side. `forward` = read as
/// "source <label> target"; otherwise as "target <label> source".
fn phrase(def: &RelationshipKindDef, custom: Option<&str>, forward: bool) -> String {
    if def.key == KIND_CUSTOM {
        // Free text cannot be inverted, so both sides see the same words.
        return custom.unwrap_or("related to").to_owned();
    }
    if forward {
        def.forward_label.clone()
    } else {
        def.inverse_label.clone()
    }
}

/// Label, symmetry and category for drawing any link kind, including the
/// non-relationship kinds that the graph can optionally show.
pub fn edge_info(kind: &str, label: Option<&str>) -> (String, bool, RelationshipCategory) {
    match kind {
        KIND_MENTION => ("mentions".into(), false, RelationshipCategory::Other),
        KIND_INVOLVED_IN => ("involved in".into(), false, RelationshipCategory::Other),
        // The label is the name of the field that holds the reference.
        KIND_FIELD_REF => (
            label.unwrap_or("references").to_owned(),
            false,
            RelationshipCategory::Other,
        ),
        _ => match catalog::find(kind) {
            Some(def) => (phrase(&def, label, true), def.symmetric, def.category),
            // Unknown kind in the file (e.g. from a newer app version).
            None => (kind.replace('_', " "), false, RelationshipCategory::Other),
        },
    }
}

/// All relationships of `entity_id`, outgoing and incoming, each phrased
/// from that entity's point of view.
pub fn list_for_entity(conn: &Connection, entity_id: &str) -> AppResult<Vec<RelationshipView>> {
    entities::get(conn, entity_id)?; // NotFound if it does not exist
    let rows = db::links::list_for_entity(conn, entity_id, &relationship_kind_keys())?;
    let mut views: Vec<RelationshipView> = Vec::with_capacity(rows.len());
    for r in rows {
        let Some(def) = catalog::find(&r.link.kind) else {
            continue;
        };
        let outgoing = r.link.source_id == entity_id;
        let (other_id, other_name, other_type) = if outgoing {
            (r.link.target_id.clone(), r.target_name, r.target_type)
        } else {
            (r.link.source_id.clone(), r.source_name, r.source_type)
        };
        views.push(RelationshipView {
            link_id: r.link.id.clone(),
            kind: def.key.clone(),
            label: phrase(&def, r.link.label.as_deref(), outgoing),
            other_id,
            other_name,
            other_type,
            outgoing,
            symmetric: def.symmetric,
            category: def.category,
        });
    }
    // Group by label, then by name, so the editor list is stable.
    views.sort_by_cached_key(|v| (v.label.to_lowercase(), v.other_name.to_lowercase()));
    Ok(views)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
pub(crate) mod tests {
    use super::*;
    use crate::db::migrations::MIGRATIONS;
    use crate::models::entity::NewEntity;
    use crate::services::documents;
    use serde_json::json;

    /// An in-memory world with foreign keys on, like a real one.
    pub fn conn() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        c.pragma_update(None, "foreign_keys", "ON").unwrap();
        MIGRATIONS.to_latest(&mut c).unwrap();
        c
    }

    /// Creates an entity and returns its id.
    pub fn mk(c: &Connection, entity_type: &str, name: &str) -> String {
        entities::create(
            c,
            NewEntity {
                entity_type: entity_type.into(),
                name: name.into(),
            },
        )
        .unwrap()
        .id
    }

    fn validation(r: AppResult<RelationshipRecord>) -> String {
        match r {
            Err(AppError::Validation(m)) => m,
            other => panic!("expected a validation error, got {other:?}"),
        }
    }

    #[test]
    fn rejects_unknown_reserved_and_self_links() {
        let c = conn();
        let a = mk(&c, "character", "Ada");
        let b = mk(&c, "character", "Bran");
        assert!(validation(create(&c, &a, &b, "nonsense", None)).contains("Unknown"));
        assert!(validation(create(&c, &a, &b, "mention", None)).contains("not relationships"));
        assert!(validation(create(&c, &a, &b, "involved_in", None)).contains("not relationships"));
        assert!(validation(create(&c, &a, &a, "sibling_of", None)).contains("itself"));
        assert!(matches!(
            create(&c, &a, "missing", "sibling_of", None),
            Err(AppError::NotFound(_))
        ));
    }

    #[test]
    fn enforces_type_constraints() {
        let c = conn();
        let ada = mk(&c, "character", "Ada");
        let town = mk(&c, "location", "Rivertown");
        let guild = mk(&c, "faction", "Guild");
        let sword = mk(&c, "item", "Sword");
        // character -> location is not a parent_of.
        assert!(validation(create(&c, &ada, &town, "parent_of", None)).contains("character"));
        assert!(validation(create(&c, &town, &guild, "member_of", None)).contains("source"));
        assert!(validation(create(&c, &ada, &town, "member_of", None)).contains("target"));
        // Valid ones go through.
        create(&c, &ada, &guild, "member_of", None).unwrap();
        create(&c, &ada, &sword, "owns", None).unwrap();
        create(&c, &sword, &town, "located_in", None).unwrap();
        create(&c, &guild, &town, "rules", None).unwrap();
        create(&c, &town, &sword, "related_to", None).unwrap();
    }

    #[test]
    fn symmetric_kinds_are_stored_once_in_canonical_order() {
        let c = conn();
        let a = mk(&c, "character", "Ada");
        let b = mk(&c, "character", "Bran");
        let first = create(&c, &a, &b, "spouse_of", None).unwrap();
        let (low, high) = if a < b { (&a, &b) } else { (&b, &a) };
        assert_eq!((&first.source_id, &first.target_id), (low, high));

        // The reverse direction and an exact repeat are both duplicates.
        assert!(validation(create(&c, &b, &a, "spouse_of", None)).contains("already"));
        assert!(validation(create(&c, &a, &b, "spouse_of", None)).contains("already"));
        let count: i64 = c
            .query_row(
                "SELECT count(*) FROM links WHERE kind = 'spouse_of'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn asymmetric_duplicates_are_rejected_but_reverse_is_a_different_link() {
        let c = conn();
        let a = mk(&c, "character", "Ada");
        let b = mk(&c, "character", "Bran");
        create(&c, &a, &b, "mentor_of", None).unwrap();
        assert!(validation(create(&c, &a, &b, "mentor_of", None)).contains("already"));
        create(&c, &b, &a, "mentor_of", None).unwrap();
    }

    #[test]
    fn custom_relationships_need_a_label_and_allow_one_per_direction() {
        let c = conn();
        let a = mk(&c, "character", "Ada");
        let b = mk(&c, "character", "Bran");
        assert!(validation(create(&c, &a, &b, "custom", None)).contains("label"));
        assert!(validation(create(&c, &a, &b, "custom", Some("  ".into()))).contains("label"));
        let long = "x".repeat(101);
        assert!(validation(create(&c, &a, &b, "custom", Some(long))).contains("at most"));

        let r = create(&c, &a, &b, "custom", Some(" owes a debt to ".into())).unwrap();
        assert_eq!(r.label.as_deref(), Some("owes a debt to"));
        assert!(validation(create(&c, &a, &b, "custom", Some("other".into()))).contains("already"));
        create(&c, &b, &a, "custom", Some("hates".into())).unwrap();

        // A label on a built-in kind is ignored.
        let p = create(&c, &a, &b, "mentor_of", Some("ignored".into())).unwrap();
        assert_eq!(p.label, None);
    }

    #[test]
    fn perspective_labels() {
        let c = conn();
        let parent = mk(&c, "character", "Parent");
        let child = mk(&c, "character", "Child");
        let spouse = mk(&c, "character", "Spouse");
        create(&c, &parent, &child, "parent_of", None).unwrap();
        create(&c, &child, &spouse, "spouse_of", None).unwrap();
        create(&c, &child, &parent, "custom", Some("owes money to".into())).unwrap();

        let p = list_for_entity(&c, &parent).unwrap();
        let parent_of = p.iter().find(|v| v.kind == "parent_of").unwrap();
        assert_eq!(parent_of.label, "parent of");
        assert_eq!(parent_of.other_name, "Child");
        assert!(parent_of.outgoing);

        let ch = list_for_entity(&c, &child).unwrap();
        let child_of = ch.iter().find(|v| v.kind == "parent_of").unwrap();
        assert_eq!(child_of.label, "child of");
        assert_eq!(child_of.other_name, "Parent");
        assert_eq!(child_of.other_type, "character");
        assert!(!child_of.outgoing);

        // Symmetric: same wording whichever side asks.
        for (me, other) in [(&child, "Spouse"), (&spouse, "Child")] {
            let v = list_for_entity(&c, me).unwrap();
            let s = v.iter().find(|v| v.kind == "spouse_of").unwrap();
            assert_eq!(
                (s.label.as_str(), s.other_name.as_str()),
                ("spouse of", other)
            );
            assert!(s.symmetric);
        }
        // Custom text is shown as written on both sides.
        assert!(ch.iter().any(|v| v.label == "owes money to"));
        assert!(p.iter().any(|v| v.label == "owes money to"));
    }

    #[test]
    fn update_label_kind_and_direction() {
        let c = conn();
        let a = mk(&c, "character", "Ada");
        let b = mk(&c, "character", "Bran");
        let link = create(&c, &a, &b, "custom", Some("rival".into())).unwrap();

        let r = update(&c, &link.id, None, Some("bitter rival".into()), None).unwrap();
        assert_eq!(r.label.as_deref(), Some("bitter rival"));

        // Swap direction.
        let r = update(&c, &link.id, None, None, Some(true)).unwrap();
        assert_eq!(
            (r.source_id.as_str(), r.target_id.as_str()),
            (b.as_str(), a.as_str())
        );
        assert_eq!(r.label.as_deref(), Some("bitter rival"));

        // Changing to a built-in kind clears the label.
        let r = update(&c, &link.id, Some("mentor_of".into()), None, None).unwrap();
        assert_eq!(r.kind, "mentor_of");
        assert_eq!(r.label, None);

        // Changing to a symmetric kind re-canonicalises the order.
        let r = update(&c, &link.id, Some("friend_of".into()), None, None).unwrap();
        assert!(r.source_id < r.target_id);

        // Constraints apply on update too.
        assert!(
            update(&c, &link.id, Some("member_of".into()), None, None).is_err(),
            "character -> character is not a membership"
        );
        // Custom without any label after a clear is rejected.
        assert!(update(&c, &link.id, Some("custom".into()), None, None).is_err());
        assert!(matches!(
            update(&c, "nope", None, None, None),
            Err(AppError::NotFound(_))
        ));
    }

    #[test]
    fn updating_into_a_duplicate_is_rejected() {
        let c = conn();
        let a = mk(&c, "character", "Ada");
        let b = mk(&c, "character", "Bran");
        create(&c, &a, &b, "friend_of", None).unwrap();
        let other = create(&c, &a, &b, "mentor_of", None).unwrap();
        assert!(update(&c, &other.id, Some("friend_of".into()), None, None).is_err());
    }

    #[test]
    fn delete_removes_only_relationships() {
        let c = conn();
        let a = mk(&c, "character", "Ada");
        let b = mk(&c, "character", "Bran");
        let r = create(&c, &a, &b, "friend_of", None).unwrap();
        delete(&c, &r.id).unwrap();
        assert!(list_for_entity(&c, &a).unwrap().is_empty());
        assert!(matches!(delete(&c, &r.id), Err(AppError::NotFound(_))));

        // A mention link cannot be edited or deleted through this module.
        db::links::insert_link(&c, &a, &b, KIND_MENTION).unwrap();
        let mention = db::links::find_link(&c, &a, &b, KIND_MENTION)
            .unwrap()
            .unwrap();
        assert!(matches!(
            delete(&c, &mention.id),
            Err(AppError::Validation(_))
        ));
        assert!(update(&c, &mention.id, None, Some("x".into()), None).is_err());
    }

    fn mention_doc(target: &str) -> String {
        json!({ "type": "doc", "content": [{ "type": "paragraph", "content": [
            { "type": "mention", "attrs": { "id": target } }
        ]}]})
        .to_string()
    }

    #[test]
    fn mention_sync_and_relationships_do_not_touch_each_other() {
        let c = conn();
        let a = mk(&c, "character", "Ada");
        let b = mk(&c, "character", "Bran");
        let rel = create(&c, &a, &b, "friend_of", None).unwrap();

        // Saving a document (which re-syncs mentions) keeps the relationship,
        // even when the document mentions the same entity.
        documents::save(&c, &a, mention_doc(&b), "Bran".into()).unwrap();
        documents::save(&c, &a, mention_doc("gone"), String::new()).unwrap();
        assert!(db::links::get_link(&c, &rel.id).unwrap().is_some());

        // And relationship edits keep the mention.
        documents::save(&c, &a, mention_doc(&b), "Bran".into()).unwrap();
        delete(&c, &rel.id).unwrap();
        assert_eq!(
            db::links::targets_for_source(&c, &a, KIND_MENTION).unwrap(),
            vec![b.clone()]
        );
        // Mentions never show up as relationships.
        assert!(list_for_entity(&c, &a).unwrap().is_empty());
        assert!(list_for_entity(&c, &b).unwrap().is_empty());
    }

    #[test]
    fn deleting_an_entity_cascades_its_relationships() {
        let c = conn();
        let a = mk(&c, "character", "Ada");
        let b = mk(&c, "character", "Bran");
        let d = mk(&c, "character", "Dara");
        create(&c, &a, &b, "parent_of", None).unwrap();
        create(&c, &b, &d, "friend_of", None).unwrap();
        entities::delete(&c, &b).unwrap();
        assert!(list_for_entity(&c, &a).unwrap().is_empty());
        assert!(list_for_entity(&c, &d).unwrap().is_empty());
        let n: i64 = c
            .query_row("SELECT count(*) FROM links", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }
}
