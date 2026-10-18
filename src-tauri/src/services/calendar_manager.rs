//! Managing a world's calendars: create, edit, duplicate, delete, choose the
//! default and the display calendar, and preview unsaved definitions.
//!
//! Important: events are stored as *absolute* day numbers and never move when
//! a calendar changes. Editing month lengths, leap rules or the epoch offset
//! only changes how those existing days are *displayed*.

use rusqlite::Connection;
use uuid::Uuid;

use crate::db;
use crate::error::{AppError, AppResult};
use crate::models::calendar::{
    Calendar, CalendarDefinition, CalendarPreview, DateStyle, ValidationIssue,
};
use crate::services::calendar::{self, MAX_NAME_LEN};

/// `world_settings` key holding the id of the calendar the world is shown in.
const DISPLAY_KEY: &str = "display_calendar_id";

/// Trims a calendar name and checks its length.
fn clean_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    let len = name.chars().count();
    if len == 0 || len > MAX_NAME_LEN {
        return Err(AppError::Validation(format!(
            "A calendar name needs 1 to {MAX_NAME_LEN} characters"
        )));
    }
    Ok(name.to_owned())
}

/// Turns every issue of a definition into one readable validation error.
fn check_definition(def: &CalendarDefinition) -> AppResult<()> {
    let issues = calendar::validate_definition(def);
    if issues.is_empty() {
        return Ok(());
    }
    let text: Vec<String> = issues
        .iter()
        .map(|i| format!("{}: {}", i.field, i.message))
        .collect();
    Err(AppError::Validation(text.join("; ")))
}

fn check_name_free(conn: &Connection, name: &str, except_id: Option<&str>) -> AppResult<()> {
    if db::calendars::name_taken(conn, name, except_id)? {
        return Err(AppError::Validation(format!(
            "A calendar called \"{name}\" already exists"
        )));
    }
    Ok(())
}

pub fn get(conn: &Connection, id: &str) -> AppResult<Calendar> {
    calendar::resolve(conn, Some(id))
}

pub fn create(
    conn: &Connection,
    name: &str,
    definition: CalendarDefinition,
) -> AppResult<Calendar> {
    let name = clean_name(name)?;
    check_definition(&definition)?;
    check_name_free(conn, &name, None)?;
    let id = Uuid::now_v7().to_string();
    db::calendars::insert(conn, &id, &name, &definition)?;
    get(conn, &id)
}

/// Changes the name and/or definition (`None` = keep).
pub fn update(
    conn: &Connection,
    id: &str,
    name: Option<&str>,
    definition: Option<CalendarDefinition>,
) -> AppResult<Calendar> {
    let current = get(conn, id)?;
    let name = match name {
        Some(n) => clean_name(n)?,
        None => current.name,
    };
    let definition = match definition {
        Some(d) => {
            check_definition(&d)?;
            d
        }
        None => current.definition,
    };
    check_name_free(conn, &name, Some(id))?;
    db::calendars::update(conn, id, &name, &definition)?;
    get(conn, id)
}

/// Copies a calendar under a new name (handy for trying out edits).
pub fn duplicate(conn: &Connection, id: &str, new_name: &str) -> AppResult<Calendar> {
    let source = get(conn, id)?;
    create(conn, new_name, source.definition)
}

/// Deletes a calendar. The default calendar (and so also the last one) cannot
/// be deleted. Events that used it are reassigned to the default calendar in
/// the same transaction, and the display setting falls back to the default.
pub fn delete(conn: &Connection, id: &str) -> AppResult<()> {
    let target = get(conn, id)?;
    if target.is_default {
        return Err(AppError::Validation(
            "The default calendar cannot be deleted. Make another calendar the default first."
                .into(),
        ));
    }
    let default = calendar::get_default(conn)?;
    // A transaction makes the steps all-or-nothing: if one fails, none stick.
    let tx = conn.unchecked_transaction()?;
    db::events::reassign_calendar(&tx, id, &default.id)?;
    if db::world_settings::get(&tx, DISPLAY_KEY)?.as_deref() == Some(id) {
        db::world_settings::delete(&tx, DISPLAY_KEY)?;
    }
    db::calendars::delete(&tx, id)?;
    tx.commit()?;
    Ok(())
}

pub fn set_default(conn: &Connection, id: &str) -> AppResult<Calendar> {
    get(conn, id)?; // NotFound if missing
    db::calendars::set_default(conn, id)?;
    get(conn, id)
}

/// The calendar the world is displayed in: the saved choice, or the default
/// calendar when nothing is saved (or the saved calendar no longer exists).
pub fn get_display(conn: &Connection) -> AppResult<Calendar> {
    if let Some(id) = db::world_settings::get(conn, DISPLAY_KEY)?
        && let Some(found) = db::calendars::get(conn, &id)?
    {
        return Ok(found);
    }
    calendar::get_default(conn)
}

/// Saves the display calendar; `None` = follow the default calendar.
pub fn set_display(conn: &Connection, calendar_id: Option<&str>) -> AppResult<Calendar> {
    match calendar_id {
        Some(id) => {
            get(conn, id)?;
            db::world_settings::set(conn, DISPLAY_KEY, id)?;
        }
        None => db::world_settings::delete(conn, DISPLAY_KEY)?,
    }
    get_display(conn)
}

/// Validates an unsaved definition and formats `days` with it. Problems are
/// returned as data (not as an error) so an editor can show them per field.
pub fn preview(def: &CalendarDefinition, days: &[i64], style: DateStyle) -> CalendarPreview {
    let issues: Vec<ValidationIssue> = calendar::validate_definition(def);
    if !issues.is_empty() {
        return CalendarPreview {
            issues,
            labels: Vec::new(),
        };
    }
    let mut labels = Vec::with_capacity(days.len());
    for &day in days {
        match calendar::format(def, day, style) {
            Ok(text) => labels.push(text),
            // An out-of-range day only affects its own label.
            Err(e) => labels.push(e.to_string()),
        }
    }
    CalendarPreview {
        issues: Vec::new(),
        labels,
    }
}

/// Days in a month of an era year (for date editors).
pub fn days_in_month(
    conn: &Connection,
    calendar_id: Option<&str>,
    era_index: u32,
    year: i64,
    month: u32,
) -> AppResult<u32> {
    let cal = calendar::resolve(conn, calendar_id)?;
    calendar::days_in_month(&cal.definition, era_index, year, month)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::db::migrations::MIGRATIONS;
    use crate::models::calendar::{Era, LeapCycle, LeapRule, Month};
    use crate::services::{entities, events};

    fn conn() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        MIGRATIONS.to_latest(&mut conn).unwrap();
        conn
    }

    fn thirteen() -> CalendarDefinition {
        CalendarDefinition {
            months: (1..=13)
                .map(|i| Month {
                    name: format!("Moon {i}"),
                    days: 28,
                })
                .collect(),
            weekdays: vec![],
            eras: vec![
                Era {
                    name: "Before".into(),
                    abbreviation: Some("BR".into()),
                    start_day: -10_000,
                },
                Era {
                    name: "Reckoning".into(),
                    abbreviation: Some("R".into()),
                    start_day: 0,
                },
            ],
            epoch_offset: 0,
            leap: Some(LeapRule {
                month_index: 12,
                extra_days: 1,
                rules: vec![
                    LeapCycle {
                        every: 4,
                        add: true,
                    },
                    LeapCycle {
                        every: 100,
                        add: false,
                    },
                ],
            }),
        }
    }

    #[test]
    fn create_get_update_and_unique_names() {
        let c = conn();
        let cal = create(&c, "  Moon Count ", thirteen()).unwrap();
        assert_eq!(cal.name, "Moon Count");
        assert!(!cal.is_default);
        assert_eq!(get(&c, &cal.id).unwrap().definition, thirteen());
        assert_eq!(db::calendars::list(&c).unwrap().len(), 2);

        // Names are unique ignoring case, here and when renaming.
        assert!(create(&c, "moon count", thirteen()).is_err());
        assert!(create(&c, "standard CALENDAR", thirteen()).is_err());
        assert!(create(&c, "   ", thirteen()).is_err());
        assert!(create(&c, &"x".repeat(61), thirteen()).is_err());
        let renamed = update(&c, &cal.id, Some("MOON COUNT"), None).unwrap();
        assert_eq!(renamed.name, "MOON COUNT"); // own name is fine
        assert!(update(&c, &cal.id, Some("Standard calendar"), None).is_err());

        let mut def = thirteen();
        def.epoch_offset = 50;
        let changed = update(&c, &cal.id, None, Some(def.clone())).unwrap();
        assert_eq!(changed.definition, def);
        assert_eq!(changed.name, "MOON COUNT");
        assert!(matches!(get(&c, "missing"), Err(AppError::NotFound(_))));
    }

    #[test]
    fn invalid_definitions_are_refused() {
        let c = conn();
        let mut bad = thirteen();
        bad.months[0].days = 0;
        let err = create(&c, "Bad", bad.clone()).unwrap_err();
        assert!(err.to_string().contains("months[0].days"), "{err}");
        let cal = create(&c, "Good", thirteen()).unwrap();
        assert!(update(&c, &cal.id, None, Some(bad)).is_err());
        assert_eq!(get(&c, &cal.id).unwrap().definition, thirteen());
    }

    #[test]
    fn duplicate_copies_the_definition() {
        let c = conn();
        let default = calendar::get_default(&c).unwrap();
        let copy = duplicate(&c, &default.id, "Copy").unwrap();
        assert_eq!(copy.definition, default.definition);
        assert!(!copy.is_default);
        assert_ne!(copy.id, default.id);
        assert!(duplicate(&c, &default.id, "copy").is_err());
        assert!(duplicate(&c, "missing", "Other").is_err());
    }

    #[test]
    fn default_calendar_cannot_be_deleted_but_others_can_after_set_default() {
        let c = conn();
        let default = calendar::get_default(&c).unwrap();
        assert!(delete(&c, &default.id).is_err()); // default and last
        let other = create(&c, "Other", thirteen()).unwrap();
        assert!(delete(&c, &default.id).is_err()); // still the default

        let promoted = set_default(&c, &other.id).unwrap();
        assert!(promoted.is_default);
        assert_eq!(calendar::get_default(&c).unwrap().id, other.id);
        assert!(!get(&c, &default.id).unwrap().is_default);
        delete(&c, &default.id).unwrap();
        assert_eq!(db::calendars::list(&c).unwrap().len(), 1);
        assert!(delete(&c, &other.id).is_err()); // last calendar
        assert!(set_default(&c, "missing").is_err());
    }

    #[test]
    fn deleting_a_calendar_reassigns_its_events_to_the_default() {
        let c = conn();
        let default = calendar::get_default(&c).unwrap();
        let other = create(&c, "Other", thirteen()).unwrap();
        let ev = events::create(&c, "Battle", 100, None).unwrap();
        events::update_dates(&c, &ev.entity_id, 100, None, None, Some(&other.id)).unwrap();
        assert_eq!(
            events::get(&c, &ev.entity_id).unwrap().calendar_id,
            other.id
        );
        set_display(&c, Some(&other.id)).unwrap();

        delete(&c, &other.id).unwrap();
        let after = events::get(&c, &ev.entity_id).unwrap();
        assert_eq!(after.calendar_id, default.id);
        assert_eq!(after.start_day, 100); // events never move
        assert_eq!(get_display(&c).unwrap().id, default.id);
        assert!(entities::get(&c, &ev.entity_id).is_ok());
    }

    #[test]
    fn editing_a_calendar_does_not_move_events() {
        let c = conn();
        let cal = create(&c, "Moon", thirteen()).unwrap();
        let ev = events::create(&c, "Feast", 12_345, None).unwrap();
        let mut def = thirteen();
        def.months[0].days = 40;
        def.epoch_offset = 999;
        update(&c, &cal.id, None, Some(def)).unwrap();
        assert_eq!(events::get(&c, &ev.entity_id).unwrap().start_day, 12_345);
    }

    #[test]
    fn display_calendar_defaults_to_the_default_calendar() {
        let c = conn();
        let default = calendar::get_default(&c).unwrap();
        assert_eq!(get_display(&c).unwrap().id, default.id);
        let other = create(&c, "Other", thirteen()).unwrap();
        assert_eq!(set_display(&c, Some(&other.id)).unwrap().id, other.id);
        assert_eq!(get_display(&c).unwrap().id, other.id);
        assert_eq!(set_display(&c, None).unwrap().id, default.id);
        assert!(set_display(&c, Some("missing")).is_err());
        // Changing the default does not change an explicit display choice.
        set_display(&c, Some(&other.id)).unwrap();
        set_default(&c, &default.id).unwrap();
        assert_eq!(get_display(&c).unwrap().id, other.id);
    }

    #[test]
    fn preview_formats_valid_and_reports_invalid_definitions() {
        let ok = preview(&thirteen(), &[0, -1], DateStyle::Full);
        assert!(ok.issues.is_empty());
        assert_eq!(ok.labels.len(), 2);
        assert_eq!(ok.labels[0], "1 Moon 1, 1 R");

        let mut bad = thirteen();
        bad.leap.as_mut().unwrap().month_index = 40;
        bad.eras.clear();
        let out = preview(&bad, &[0], DateStyle::Full);
        assert!(out.labels.is_empty());
        let fields: Vec<&str> = out.issues.iter().map(|i| i.field.as_str()).collect();
        assert!(fields.contains(&"eras"));
        assert!(fields.contains(&"leap.monthIndex"));
    }

    #[test]
    fn days_in_month_follows_leap_years() {
        let c = conn();
        let cal = create(&c, "Moon", thirteen()).unwrap();
        // Era 1 starts at day 0 = year index 0 (year number 1): era year 4
        // is year number 4, a leap year, so the 13th month has 29 days.
        assert_eq!(days_in_month(&c, Some(&cal.id), 1, 4, 13).unwrap(), 29);
        assert_eq!(days_in_month(&c, Some(&cal.id), 1, 3, 13).unwrap(), 28);
        assert_eq!(days_in_month(&c, None, 0, 1, 12).unwrap(), 30);
        assert!(days_in_month(&c, Some("missing"), 0, 1, 1).is_err());
    }
}
