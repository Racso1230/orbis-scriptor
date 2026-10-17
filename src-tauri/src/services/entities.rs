//! Entity business rules: validation, IDs and timestamps. SQL lives in
//! `db::entities`.

use chrono::{SecondsFormat, Utc};
use rusqlite::Connection;
use serde_json::Value;
use uuid::Uuid;

use crate::db;
use crate::error::{AppError, AppResult};
use crate::models::entity::{Entity, EntityPatch, EntitySummary, NewEntity};
use crate::models::entity_type;
use crate::services::{events, maps, search};

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

fn validate_type(entity_type: &str) -> AppResult<()> {
    if entity_type::is_valid(entity_type) {
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
    create_with_dates(conn, input, 0, None)
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
    create_with_dates(conn, input, start_day, end_day)
}

/// Shared by both functions above. The dates are only used for `event`
/// entities and ignored for every other type.
fn create_with_dates(
    conn: &Connection,
    input: NewEntity,
    start_day: i64,
    end_day: Option<i64>,
) -> AppResult<Entity> {
    validate_type(&input.entity_type)?;
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
    };
    // Row and search-index entry are written together or not at all.
    // `unchecked_transaction` works on a shared `&Connection`.
    let tx = conn.unchecked_transaction()?;
    db::entities::insert(&tx, &entity)?;
    // A `map` entity also needs its `maps` row and default layer.
    if entity.entity_type == "map" {
        maps::create_for_entity(&tx, &entity.id)?;
    }
    // An `event` entity needs its `events` row (default calendar).
    if entity.entity_type == "event" {
        events::create_for_entity(&tx, &entity.id, start_day, end_day)?;
    }
    search::reindex_entity(&tx, &entity.id)?;
    tx.commit()?;
    Ok(entity)
}

pub fn get(conn: &Connection, id: &str) -> AppResult<Entity> {
    db::entities::get(conn, id)?
        .ok_or_else(|| AppError::NotFound(format!("Entity {id} does not exist")))
}

pub fn list(conn: &Connection, entity_type: Option<&str>) -> AppResult<Vec<EntitySummary>> {
    if let Some(t) = entity_type {
        validate_type(t)?;
    }
    db::entities::list(conn, entity_type)
}

/// Applies the `Some` fields of `patch` and returns the updated entity.
pub fn update(conn: &Connection, id: &str, patch: EntityPatch) -> AppResult<Entity> {
    let mut entity = get(conn, id)?;
    if let Some(name) = patch.name {
        entity.name = validate_name(&name)?;
    }
    if let Some(summary) = patch.summary {
        entity.summary = summary;
    }
    if let Some(properties) = patch.properties {
        validate_properties(&properties)?;
        entity.properties = properties;
    }
    entity.updated_at = now();
    let tx = conn.unchecked_transaction()?;
    db::entities::update(&tx, &entity)?;
    search::reindex_entity(&tx, &entity.id)?;
    tx.commit()?;
    Ok(entity)
}

pub fn delete(conn: &Connection, id: &str) -> AppResult<()> {
    let tx = conn.unchecked_transaction()?;
    db::entities::delete(&tx, id)?;
    search::remove_entity(&tx, id)?;
    tx.commit()?;
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
