//! Timeline events. An event is an entity (type `event`) plus an `events`
//! row with its dates; "involved" entities are `involved_in` links
//! (source = the involved entity, target = the event).

use rusqlite::Connection;

use crate::db;
use crate::error::{AppError, AppResult};
use crate::models::entity::{EntitySummary, NewEntity};
use crate::models::event::{EventSummary, MAX_IMPORTANCE};
use crate::models::link::KIND_INVOLVED_IN;
use crate::services::{calendar, entities};

/// Importance given to new events.
const DEFAULT_IMPORTANCE: i32 = 1;

/// Checks day numbers and that the range does not end before it starts.
fn validate_dates(start_day: i64, end_day: Option<i64>) -> AppResult<()> {
    calendar::check_day(start_day)?;
    if let Some(end) = end_day {
        calendar::check_day(end)?;
        if end < start_day {
            return Err(AppError::Validation(
                "An event cannot end before it starts".into(),
            ));
        }
    }
    Ok(())
}

fn validate_importance(importance: i32) -> AppResult<()> {
    if (1..=MAX_IMPORTANCE).contains(&importance) {
        Ok(())
    } else {
        Err(AppError::Validation(format!(
            "Importance must be between 1 and {MAX_IMPORTANCE}"
        )))
    }
}

/// Inserts the `events` row for an event entity that was just created, using
/// the default calendar. Called by `entities` inside its transaction.
pub fn create_for_entity(
    conn: &Connection,
    entity_id: &str,
    start_day: i64,
    end_day: Option<i64>,
) -> AppResult<()> {
    validate_dates(start_day, end_day)?;
    let calendar = calendar::get_default(conn)?;
    db::events::insert(
        conn,
        entity_id,
        &calendar.id,
        start_day,
        end_day,
        DEFAULT_IMPORTANCE,
    )
}

/// Creates the event entity and its dates in one transaction (the entity
/// service also keeps the search index in sync).
pub fn create(
    conn: &Connection,
    name: &str,
    start_day: i64,
    end_day: Option<i64>,
) -> AppResult<EventSummary> {
    let entity = entities::create_event(
        conn,
        NewEntity {
            entity_type: "event".into(),
            name: name.into(),
        },
        start_day,
        end_day,
    )?;
    get(conn, &entity.id)
}

/// Like [`create`] but WITHOUT its own transaction, and with an importance
/// and the calendar the dates were made in. The caller must already be
/// inside a transaction (the world generator writes everything in one).
pub fn create_in(
    conn: &Connection,
    name: &str,
    start_day: i64,
    end_day: Option<i64>,
    importance: i32,
    calendar_id: &str,
) -> AppResult<EventSummary> {
    validate_importance(importance)?;
    calendar::resolve(conn, Some(calendar_id))?;
    let entity = entities::create_event_in(
        conn,
        NewEntity {
            entity_type: "event".into(),
            name: name.into(),
        },
        start_day,
        end_day,
    )?;
    db::events::update_dates(conn, &entity.id, start_day, end_day, Some(importance))?;
    db::events::set_calendar(conn, &entity.id, calendar_id)?;
    get(conn, &entity.id)
}

pub fn get(conn: &Connection, entity_id: &str) -> AppResult<EventSummary> {
    db::events::get(conn, entity_id)?
        .ok_or_else(|| AppError::NotFound(format!("Event {entity_id} does not exist")))
}

/// Events overlapping `[from_day, to_day]`.
pub fn list(conn: &Connection, from_day: i64, to_day: i64) -> AppResult<Vec<EventSummary>> {
    if to_day < from_day {
        return Err(AppError::Validation(
            "The window must not end before it starts".into(),
        ));
    }
    db::events::list_range(conn, from_day, to_day)
}

pub fn update_dates(
    conn: &Connection,
    entity_id: &str,
    start_day: i64,
    end_day: Option<i64>,
    importance: Option<i32>,
    calendar_id: Option<&str>,
) -> AppResult<EventSummary> {
    validate_dates(start_day, end_day)?;
    if let Some(i) = importance {
        validate_importance(i)?;
    }
    // Record which calendar the dates were entered in (display hint only:
    // the stored days are absolute and do not depend on it).
    if let Some(id) = calendar_id {
        calendar::resolve(conn, Some(id))?;
    }
    db::events::update_dates(conn, entity_id, start_day, end_day, importance)?;
    if let Some(id) = calendar_id {
        db::events::set_calendar(conn, entity_id, id)?;
    }
    // New dates are an edit of the event ("recently edited" list).
    db::entities::touch(
        conn,
        entity_id,
        &chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
    )?;
    get(conn, entity_id)
}

// ---- Involved entities ----

/// Entities involved in the event.
pub fn list_involved(conn: &Connection, event_id: &str) -> AppResult<Vec<EntitySummary>> {
    get(conn, event_id)?; // NotFound if it is not an event
    db::links::list_sources(conn, event_id, KIND_INVOLVED_IN)
}

pub fn add_involved(conn: &Connection, event_id: &str, entity_id: &str) -> AppResult<()> {
    get(conn, event_id)?;
    if event_id == entity_id {
        return Err(AppError::Validation(
            "An event cannot be involved in itself".into(),
        ));
    }
    entities::get(conn, entity_id)?;
    db::links::insert_link(conn, entity_id, event_id, KIND_INVOLVED_IN)
}

pub fn remove_involved(conn: &Connection, event_id: &str, entity_id: &str) -> AppResult<()> {
    db::links::delete_link(conn, entity_id, event_id, KIND_INVOLVED_IN)?;
    Ok(())
}

/// Events the given entity is involved in, oldest first.
pub fn list_for_entity(conn: &Connection, entity_id: &str) -> AppResult<Vec<EventSummary>> {
    entities::get(conn, entity_id)?;
    db::events::list_for_entity(conn, entity_id)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::db::migrations::MIGRATIONS;
    use crate::models::link::KIND_MENTION;
    use crate::services::{documents, links};
    use serde_json::json;

    fn conn() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        c.pragma_update(None, "foreign_keys", "ON").unwrap();
        MIGRATIONS.to_latest(&mut c).unwrap();
        c
    }

    fn character(c: &Connection, name: &str) -> String {
        entities::create(
            c,
            NewEntity {
                entity_type: "character".into(),
                name: name.into(),
            },
        )
        .unwrap()
        .id
    }

    #[test]
    fn creating_an_event_entity_creates_its_row_with_the_default_calendar() {
        let c = conn();
        let e = entities::create(
            &c,
            NewEntity {
                entity_type: "event".into(),
                name: "Founding".into(),
            },
        )
        .unwrap();
        let ev = get(&c, &e.id).unwrap();
        assert_eq!(ev.name, "Founding");
        assert_eq!((ev.start_day, ev.end_day, ev.importance), (0, None, 1));
        assert_eq!(ev.calendar_id, calendar::get_default(&c).unwrap().id);
    }

    #[test]
    fn create_event_with_dates_is_searchable_and_validated() {
        let c = conn();
        let ev = create(&c, "  The Long War ", -500, Some(120)).unwrap();
        assert_eq!(ev.name, "The Long War");
        assert_eq!((ev.start_day, ev.end_day), (-500, Some(120)));
        assert_eq!(
            crate::services::search::search(&c, "long", 10).unwrap()[0].entity_id,
            ev.entity_id
        );

        assert!(matches!(
            create(&c, "Bad", 10, Some(5)),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            create(&c, "  ", 10, None),
            Err(AppError::Validation(_))
        ));
        // Nothing half-created by the failed calls.
        let count: i64 = c
            .query_row("SELECT count(*) FROM entities", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn deleting_the_entity_removes_the_event_row() {
        let c = conn();
        let ev = create(&c, "Gone", 1, None).unwrap();
        entities::delete(&c, &ev.entity_id).unwrap();
        let rows: i64 = c
            .query_row("SELECT count(*) FROM events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 0);
    }

    #[test]
    fn range_query_returns_overlapping_events_in_order() {
        let c = conn();
        let point_in = create(&c, "point in", 50, None).unwrap();
        create(&c, "point before", 5, None).unwrap();
        create(&c, "point after", 500, None).unwrap();
        let edge = create(&c, "ends on window start", -20, Some(10)).unwrap();
        let edge_after = create(&c, "starts on window end", 100, Some(150)).unwrap();
        create(&c, "ends just before", -20, Some(9)).unwrap();
        let spanning = create(&c, "spans window", -1000, Some(1000)).unwrap();
        let negative = create(&c, "negative", -5, None).unwrap();

        let ids: Vec<String> = list(&c, 10, 100)
            .unwrap()
            .into_iter()
            .map(|e| e.entity_id)
            .collect();
        assert_eq!(
            ids,
            vec![
                spanning.entity_id.clone(),
                edge.entity_id.clone(),
                point_in.entity_id.clone(),
                edge_after.entity_id.clone()
            ]
        );

        // A window entirely in negative days.
        let ids: Vec<String> = list(&c, -10, -1)
            .unwrap()
            .into_iter()
            .map(|e| e.entity_id)
            .collect();
        assert!(ids.contains(&negative.entity_id));
        assert!(ids.contains(&spanning.entity_id));
        assert!(!ids.contains(&point_in.entity_id));

        assert!(matches!(list(&c, 5, 4), Err(AppError::Validation(_))));
    }

    #[test]
    fn update_dates_validates_and_keeps_importance_when_omitted() {
        let c = conn();
        let ev = create(&c, "Coronation", 10, None).unwrap();

        let updated = update_dates(&c, &ev.entity_id, 20, Some(25), Some(4), None).unwrap();
        assert_eq!(
            (updated.start_day, updated.end_day, updated.importance),
            (20, Some(25), 4)
        );
        let updated = update_dates(&c, &ev.entity_id, -3, None, None, None).unwrap();
        assert_eq!(
            (updated.start_day, updated.end_day, updated.importance),
            (-3, None, 4)
        );

        assert!(matches!(
            update_dates(&c, &ev.entity_id, 10, Some(9), None, None),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            update_dates(&c, &ev.entity_id, 10, None, Some(6), None),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            update_dates(&c, &ev.entity_id, 10, None, Some(0), None),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            update_dates(&c, "missing", 1, None, None, None),
            Err(AppError::NotFound(_))
        ));
        // The failed updates changed nothing.
        assert_eq!(get(&c, &ev.entity_id).unwrap().start_day, -3);
    }

    #[test]
    fn involved_entities_add_list_remove() {
        let c = conn();
        let ev = create(&c, "Coronation", 10, None).unwrap();
        let ada = character(&c, "Ada");
        let bob = character(&c, "Bob");

        add_involved(&c, &ev.entity_id, &ada).unwrap();
        add_involved(&c, &ev.entity_id, &ada).unwrap(); // duplicate is a no-op
        add_involved(&c, &ev.entity_id, &bob).unwrap();
        let names: Vec<String> = list_involved(&c, &ev.entity_id)
            .unwrap()
            .into_iter()
            .map(|e| e.name)
            .collect();
        assert_eq!(names, vec!["Ada", "Bob"]);

        let events = list_for_entity(&c, &ada).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].entity_id, ev.entity_id);

        remove_involved(&c, &ev.entity_id, &ada).unwrap();
        remove_involved(&c, &ev.entity_id, &ada).unwrap(); // already gone: fine
        assert_eq!(list_involved(&c, &ev.entity_id).unwrap().len(), 1);

        assert!(matches!(
            add_involved(&c, &ev.entity_id, &ev.entity_id),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            add_involved(&c, &ada, &bob),
            Err(AppError::NotFound(_))
        ));
        assert!(matches!(
            add_involved(&c, &ev.entity_id, "missing"),
            Err(AppError::NotFound(_))
        ));

        // Deleting the involved entity drops its link.
        entities::delete(&c, &bob).unwrap();
        assert!(list_involved(&c, &ev.entity_id).unwrap().is_empty());
    }

    #[test]
    fn mention_sync_and_backlinks_leave_involved_links_alone() {
        let c = conn();
        let ev = create(&c, "Coronation", 10, None).unwrap();
        let ada = character(&c, "Ada");
        add_involved(&c, &ev.entity_id, &ada).unwrap();

        // Ada's lore mentions the event, then stops mentioning it.
        let doc = |ids: &[&str]| {
            json!({ "type": "doc", "content": [{ "type": "paragraph", "content":
                ids.iter().map(|id| json!({ "type": "mention", "attrs": { "id": id } }))
                    .collect::<Vec<_>>() }]})
            .to_string()
        };
        documents::save(&c, &ada, doc(&[&ev.entity_id]), String::new()).unwrap();
        assert_eq!(
            db::links::targets_for_source(&c, &ada, KIND_MENTION).unwrap(),
            vec![ev.entity_id.clone()]
        );
        // Backlinks only show mentions, and the involved link is still there.
        assert_eq!(
            links::backlinks(&c, &ev.entity_id, "Coronation")
                .unwrap()
                .len(),
            1
        );

        documents::save(&c, &ada, doc(&[]), String::new()).unwrap();
        assert!(
            db::links::targets_for_source(&c, &ada, KIND_MENTION)
                .unwrap()
                .is_empty()
        );
        assert!(
            links::backlinks(&c, &ev.entity_id, "Coronation")
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            db::links::targets_for_source(&c, &ada, KIND_INVOLVED_IN).unwrap(),
            vec![ev.entity_id.clone()]
        );
        assert_eq!(list_involved(&c, &ev.entity_id).unwrap().len(), 1);
    }
}
