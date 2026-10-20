//! Entity business rules: validation, IDs and timestamps. SQL lives in
//! `db::entities`.

use std::path::Path;

use chrono::{SecondsFormat, Utc};
use rusqlite::Connection;
use serde_json::Value;
use uuid::Uuid;

use crate::db;
use crate::error::{AppError, AppResult};
use crate::models::entity::{Entity, EntityPatch, EntitySummary, NewEntity};
use crate::models::map::MapBackground;
use crate::services::{entity_types, events, gallery, maps, properties, search};

/// UTC ISO-8601 with milliseconds and a `Z` suffix.
fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn validate_name(name: &str) -> AppResult<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation("Name is required".into()));
    }
    Ok(trimmed.to_owned())
}

/// A type is valid when it is built in or a custom type of this world.
fn validate_type(conn: &Connection, entity_type: &str) -> AppResult<()> {
    if entity_types::is_known(conn, entity_type)? {
        Ok(())
    } else {
        Err(AppError::Validation(format!(
            "Unknown entity type \"{entity_type}\""
        )))
    }
}

fn validate_properties(properties: &Value) -> AppResult<()> {
    if properties.is_object() {
        Ok(())
    } else {
        Err(AppError::Validation(
            "Properties must be a JSON object".into(),
        ))
    }
}

pub fn create(conn: &Connection, input: NewEntity) -> AppResult<Entity> {
    // A plain `event` created here starts on day 0 with no end.
    create_with_dates(conn, input, 0, None, None)
}

/// Like [`create`] but WITHOUT its own transaction: the caller must
/// already be inside one (the world generator creates many entities in a
/// single transaction).
pub fn create_in(conn: &Connection, input: NewEntity) -> AppResult<Entity> {
    insert_with_dates(conn, input, 0, None, None)
}

/// Creates a `map` entity without an image: the `maps` row gets this size
/// and background look instead of 0x0. Entity, map row, default layer and
/// search entry are committed together.
pub fn create_blank_map(
    conn: &Connection,
    name: &str,
    width: u32,
    height: u32,
    background: &MapBackground,
) -> AppResult<Entity> {
    maps::validate_blank(width, height, background)?;
    let input = NewEntity {
        entity_type: "map".into(),
        name: name.into(),
    };
    create_with_dates(conn, input, 0, None, Some((width, height, background)))
}

/// Like [`create_blank_map`] but WITHOUT its own transaction: the caller
/// must already be inside one (the map generator writes the entity, the map
/// and thousands of objects in a single transaction).
pub fn create_blank_map_in(
    conn: &Connection,
    name: &str,
    width: u32,
    height: u32,
    background: &MapBackground,
) -> AppResult<Entity> {
    maps::validate_blank(width, height, background)?;
    let input = NewEntity {
        entity_type: "map".into(),
        name: name.into(),
    };
    insert_with_dates(conn, input, 0, None, Some((width, height, background)))
}

/// Creates an `event` entity with the given dates in one transaction.
pub fn create_event(
    conn: &Connection,
    input: NewEntity,
    start_day: i64,
    end_day: Option<i64>,
) -> AppResult<Entity> {
    if input.entity_type != "event" {
        return Err(AppError::Validation("Not an event entity".into()));
    }
    create_with_dates(conn, input, start_day, end_day, None)
}

/// Shared by both functions above. The dates are only used for `event`
/// entities and ignored for every other type; `blank_map` (size and look) is
/// only used for `map` entities.
fn create_with_dates(
    conn: &Connection,
    input: NewEntity,
    start_day: i64,
    end_day: Option<i64>,
    blank_map: Option<(u32, u32, &MapBackground)>,
) -> AppResult<Entity> {
    // Row and search-index entry are written together or not at all.
    // `unchecked_transaction` works on a shared `&Connection`.
    let tx = conn.unchecked_transaction()?;
    let entity = insert_with_dates(&tx, input, start_day, end_day, blank_map)?;
    tx.commit()?;
    Ok(entity)
}

/// The body of [`create_with_dates`], without a transaction of its own.
fn insert_with_dates(
    conn: &Connection,
    input: NewEntity,
    start_day: i64,
    end_day: Option<i64>,
    blank_map: Option<(u32, u32, &MapBackground)>,
) -> AppResult<Entity> {
    validate_type(conn, &input.entity_type)?;
    let name = validate_name(&input.name)?;
    let timestamp = now();
    let entity = Entity {
        // UUID v7 is time-ordered, so newer rows sort after older ones.
        id: Uuid::now_v7().to_string(),
        entity_type: input.entity_type,
        name,
        summary: String::new(),
        properties: Value::Object(Default::default()),
        created_at: timestamp.clone(),
        updated_at: timestamp,
        favorite: false,
        tags: Vec::new(),
        portrait: None,
    };
    db::entities::insert(conn, &entity)?;
    // A `map` entity also needs its `maps` row and default layer.
    if entity.entity_type == "map" {
        match blank_map {
            Some((width, height, background)) => {
                maps::create_blank_for_entity(conn, &entity.id, width, height, background)?;
            }
            None => maps::create_for_entity(conn, &entity.id)?,
        }
    }
    // An `event` entity needs its `events` row (default calendar).
    if entity.entity_type == "event" {
        events::create_for_entity(conn, &entity.id, start_day, end_day)?;
    }
    search::reindex_entity(conn, &entity.id)?;
    Ok(entity)
}

pub fn get(conn: &Connection, id: &str) -> AppResult<Entity> {
    db::entities::get(conn, id)?
        .ok_or_else(|| AppError::NotFound(format!("Entity {id} does not exist")))
}

pub fn list(conn: &Connection, entity_type: Option<&str>) -> AppResult<Vec<EntitySummary>> {
    if let Some(t) = entity_type {
        validate_type(conn, t)?;
    }
    db::entities::list(conn, entity_type)
}

/// Applies the `Some` fields of `patch` and returns the updated entity.
pub fn update(conn: &Connection, id: &str, patch: EntityPatch) -> AppResult<Entity> {
    let tx = conn.unchecked_transaction()?;
    let entity = update_in(&tx, id, patch)?;
    tx.commit()?;
    Ok(entity)
}

/// Like [`update`] but WITHOUT its own transaction: the caller must already
/// be inside one.
pub fn update_in(conn: &Connection, id: &str, patch: EntityPatch) -> AppResult<Entity> {
    let mut entity = get(conn, id)?;
    if let Some(name) = patch.name {
        entity.name = validate_name(&name)?;
    }
    if let Some(summary) = patch.summary {
        entity.summary = summary;
    }
    // Type-specific fields (`properties`) are checked against the field
    // definitions of the entity's type (built-in plus the user's own).
    let mut fields = None;
    if let Some(new_properties) = patch.properties {
        validate_properties(&new_properties)?;
        let defs = entity_types::fields_for(conn, &entity.entity_type)?;
        properties::validate_changes(conn, &defs, &entity.properties, &new_properties)?;
        entity.properties = new_properties;
        fields = Some(defs);
    }
    entity.updated_at = now();
    db::entities::update(conn, &entity)?;
    // Entity-reference fields are mirrored as `field_ref` links.
    if let Some(defs) = &fields {
        properties::sync_field_refs(conn, &entity.id, defs, &entity.properties)?;
    }
    search::reindex_entity(conn, &entity.id)?;
    Ok(entity)
}

/// Stars or un-stars an entity. This is not an edit: `updated_at` stays.
pub fn set_favorite(conn: &Connection, id: &str, favorite: bool) -> AppResult<()> {
    db::entities::set_favorite(conn, id, favorite)
}

/// Starred entities, sorted by type then name.
pub fn list_favorites(conn: &Connection) -> AppResult<Vec<EntitySummary>> {
    db::entities::list_favorites(conn)
}

/// Largest number of entities `list_recent` returns.
pub const MAX_RECENT: u32 = 100;

/// The most recently edited entities, newest first (`limit` is clamped to
/// 1..=100). An entity counts as edited when its name, summary or fields
/// change, its lore is saved, its event dates change or its tags or gallery
/// change. Starring, relationships and map pins do not count.
pub fn list_recent(conn: &Connection, limit: u32) -> AppResult<Vec<EntitySummary>> {
    db::entities::list_recent(conn, limit.clamp(1, MAX_RECENT))
}

pub fn delete(conn: &Connection, id: &str) -> AppResult<()> {
    let tx = conn.unchecked_transaction()?;
    db::entities::delete(&tx, id)?;
    search::remove_entity(&tx, id)?;
    tx.commit()?;
    Ok(())
}

/// Like [`delete`], and afterwards removes the gallery images' assets (row
/// and files) that nothing else uses (see `services::gallery`).
pub fn delete_with_assets(conn: &Connection, assets_dir: &Path, id: &str) -> AppResult<()> {
    let asset_ids = db::gallery::asset_ids_for_entity(conn, id)?;
    delete(conn, id)?;
    for asset_id in asset_ids {
        gallery::delete_asset_if_unused(conn, assets_dir, &asset_id)?;
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::db::migrations::MIGRATIONS;
    use serde_json::json;

    fn conn() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        MIGRATIONS.to_latest(&mut c).unwrap();
        c
    }

    fn new(entity_type: &str, name: &str) -> NewEntity {
        NewEntity {
            entity_type: entity_type.into(),
            name: name.into(),
        }
    }

    #[test]
    fn create_trims_name_and_sets_defaults() {
        let c = conn();
        let e = create(&c, new("character", "  Ada  ")).unwrap();
        assert_eq!(e.name, "Ada");
        assert_eq!(e.summary, "");
        assert_eq!(e.properties, json!({}));
        assert_eq!(e.created_at, e.updated_at);
        assert!(Uuid::parse_str(&e.id).is_ok());
        assert_eq!(get(&c, &e.id).unwrap(), e);
    }

    #[test]
    fn create_rejects_empty_name_and_unknown_type() {
        let c = conn();
        assert!(matches!(
            create(&c, new("character", "   ")),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            create(&c, new("dragon", "Smaug")),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn update_applies_only_given_fields() {
        let c = conn();
        let e = create(&c, new("location", "Rivertown")).unwrap();

        let updated = update(
            &c,
            &e.id,
            EntityPatch {
                properties: Some(json!({ "kind": "town", "region": "North" })),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(updated.name, "Rivertown");
        assert_eq!(updated.properties["kind"], "town");

        let updated = update(
            &c,
            &e.id,
            EntityPatch {
                name: Some("River Town".into()),
                summary: Some("A town by the river".into()),
                properties: None,
            },
        )
        .unwrap();
        assert_eq!(updated.name, "River Town");
        assert_eq!(updated.properties["region"], "North");
    }

    #[test]
    fn update_rejects_bad_input_and_missing_entity() {
        let c = conn();
        let e = create(&c, new("item", "Sword")).unwrap();
        assert!(matches!(
            update(
                &c,
                &e.id,
                EntityPatch {
                    properties: Some(json!([1, 2])),
                    ..Default::default()
                }
            ),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            update(
                &c,
                &e.id,
                EntityPatch {
                    name: Some(" ".into()),
                    ..Default::default()
                }
            ),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            update(&c, "missing", EntityPatch::default()),
            Err(AppError::NotFound(_))
        ));
    }

    #[test]
    fn list_and_delete() {
        let c = conn();
        create(&c, new("note", "B")).unwrap();
        let a = create(&c, new("note", "A")).unwrap();
        create(&c, new("faction", "Guild")).unwrap();

        assert_eq!(list(&c, None).unwrap().len(), 3);
        assert_eq!(list(&c, Some("note")).unwrap()[0].name, "A");
        assert!(matches!(
            list(&c, Some("nope")),
            Err(AppError::Validation(_))
        ));

        delete(&c, &a.id).unwrap();
        assert!(matches!(get(&c, &a.id), Err(AppError::NotFound(_))));
    }

    /// Timestamps have millisecond resolution; wait so two edits differ.
    fn tick() {
        std::thread::sleep(std::time::Duration::from_millis(3));
    }

    fn recent_names(c: &Connection, limit: u32) -> Vec<String> {
        list_recent(c, limit)
            .unwrap()
            .into_iter()
            .map(|e| e.name)
            .collect()
    }

    #[test]
    fn favourites_can_be_set_listed_and_do_not_count_as_edits() {
        let c = conn();
        let a = create(&c, new("note", "Alpha")).unwrap();
        let b = create(&c, new("character", "Beta")).unwrap();
        assert!(list_favorites(&c).unwrap().is_empty());
        tick();
        set_favorite(&c, &b.id, true).unwrap();
        let favs = list_favorites(&c).unwrap();
        assert_eq!(favs.len(), 1);
        assert!(favs[0].favorite);
        assert_eq!(favs[0].updated_at, b.updated_at);
        assert!(get(&c, &b.id).unwrap().favorite);
        assert!(
            !list(&c, None)
                .unwrap()
                .iter()
                .any(|e| e.id == a.id && e.favorite)
        );
        set_favorite(&c, &b.id, false).unwrap();
        assert!(list_favorites(&c).unwrap().is_empty());
        assert!(matches!(
            set_favorite(&c, "missing", true),
            Err(AppError::NotFound(_))
        ));
    }

    #[test]
    fn recent_lists_the_latest_edits_first() {
        use crate::services::{documents, events};
        let c = conn();
        let a = create(&c, new("note", "A")).unwrap();
        tick();
        let b = create(&c, new("note", "B")).unwrap();
        tick();
        let ev = events::create(&c, "Battle", 5, None).unwrap();
        tick();
        assert_eq!(recent_names(&c, 10), vec!["Battle", "B", "A"]);

        // Saving lore counts as an edit of the entity.
        documents::save(
            &c,
            &a.id,
            r#"{"type":"doc","content":[]}"#.into(),
            String::new(),
        )
        .unwrap();
        assert_eq!(recent_names(&c, 10), vec!["A", "Battle", "B"]);
        tick();
        // So do field/summary edits and event date changes.
        update(
            &c,
            &b.id,
            EntityPatch {
                summary: Some("x".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(recent_names(&c, 10)[0], "B");
        tick();
        events::update_dates(&c, &ev.entity_id, 6, None, None, None).unwrap();
        assert_eq!(recent_names(&c, 10), vec!["Battle", "B", "A"]);
        // The limit applies (and is clamped to at least 1).
        assert_eq!(recent_names(&c, 2), vec!["Battle", "B"]);
        assert_eq!(recent_names(&c, 0).len(), 1);
    }

    #[test]
    fn creating_a_map_entity_creates_map_and_default_layer() {
        let mut c = Connection::open_in_memory().unwrap();
        c.pragma_update(None, "foreign_keys", "ON").unwrap();
        MIGRATIONS.to_latest(&mut c).unwrap();

        let e = create(&c, new("map", "Overworld")).unwrap();
        let info = crate::db::maps::get_map_by_entity(&c, &e.id)
            .unwrap()
            .unwrap();
        assert_eq!((info.width, info.height), (0, 0));
        assert!(info.background.is_none());
        let layers = crate::db::maps::list_layers(&c, &info.id).unwrap();
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].name, "Default");
        assert_eq!(layers[0].sort_order, 0);

        // Deleting the map entity cascades to maps and layers.
        delete(&c, &e.id).unwrap();
        let maps: i64 = c
            .query_row("SELECT count(*) FROM maps", [], |r| r.get(0))
            .unwrap();
        let layers: i64 = c
            .query_row("SELECT count(*) FROM map_layers", [], |r| r.get(0))
            .unwrap();
        assert_eq!((maps, layers), (0, 0));
    }
}
