//! Development helper that fills a world with generated entities so search
//! can be tried with ~1,000 rows. Only reachable from the debug-only guard in
//! `commands::search::dev_seed_entities`.

use chrono::{SecondsFormat, Utc};
use rusqlite::Connection;
use serde_json::json;
use uuid::Uuid;

use crate::db;
use crate::error::{AppError, AppResult};
use crate::models::document::Document;
use crate::models::entity::Entity;
use crate::models::link::KIND_INVOLVED_IN;
use crate::models::map::{MapGeometry, NewMapObject};
use crate::services::maps;

const TYPES: [&str; 5] = ["character", "location", "faction", "item", "note"];
const FIRST: [&str; 12] = [
    "Aldric", "Brynn", "Caelum", "Dara", "Élan", "Fenwick", "Galen", "Hesper", "Isolde", "Joren",
    "Kestrel", "Lyra",
];
const SECOND: [&str; 12] = [
    "Stormwind",
    "Ashford",
    "Brightwater",
    "Dunmere",
    "Evenfall",
    "Frostmark",
    "Greywatch",
    "Highcrown",
    "Ironvale",
    "Moonshadow",
    "Oakenshield",
    "Ravenhold",
];
const WORDS: [&str; 16] = [
    "dragon", "river", "ancient", "prophecy", "merchant", "war", "temple", "silver", "forest",
    "crown", "shadow", "harvest", "oath", "mountain", "storm", "legend",
];

/// Tiny deterministic pseudo-random generator (a "linear congruential
/// generator"), so we don't need the `rand` crate for throwaway test data.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self, bound: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        // Use the high bits: the low bits of an LCG are not very random.
        ((self.0 >> 33) as usize) % bound
    }

    fn sentence(&mut self, words: usize) -> String {
        let parts: Vec<&str> = (0..words).map(|_| WORDS[self.next(WORDS.len())]).collect();
        parts.join(" ")
    }
}

/// Inserts `count` entities (with summaries and, for most, a lore document)
/// in a single transaction and returns how many were created.
pub fn seed(conn: &Connection, count: u32) -> AppResult<u32> {
    let mut rng = Lcg(Uuid::now_v7().as_u128() as u64);
    let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let tx = conn.unchecked_transaction()?;

    for i in 0..count {
        let name = format!(
            "{} {} {}",
            FIRST[rng.next(FIRST.len())],
            SECOND[rng.next(SECOND.len())],
            i
        );
        let entity = Entity {
            id: Uuid::now_v7().to_string(),
            entity_type: TYPES[rng.next(TYPES.len())].to_owned(),
            summary: format!("The {}", rng.sentence(4)),
            name,
            properties: json!({}),
            created_at: timestamp.clone(),
            updated_at: timestamp.clone(),
        };
        db::entities::insert(&tx, &entity)?;

        // Roughly three entities in four get some lore.
        let mut plain_text = String::new();
        if rng.next(4) != 0 {
            let paragraphs: Vec<String> = (0..1 + rng.next(3))
                .map(|_| format!("Tales speak of {}.", rng.sentence(12)))
                .collect();
            plain_text = paragraphs.join("\n");
            let content = json!({
                "type": "doc",
                "content": paragraphs.iter().map(|p| json!({
                    "type": "paragraph",
                    "content": [{ "type": "text", "text": p }]
                })).collect::<Vec<_>>()
            });
            db::documents::upsert(
                &tx,
                &Document {
                    entity_id: entity.id.clone(),
                    content: content.to_string(),
                    plain_text: plain_text.clone(),
                    updated_at: timestamp.clone(),
                },
            )?;
        }
        db::search::replace(&tx, &entity.id, &entity.name, &entity.summary, &plain_text)?;
    }

    tx.commit()?;
    Ok(count)
}

/// Size used for pin positions when the map has no background yet.
const FALLBACK_MAP_SIZE: (u32, u32) = (2000, 1500);

/// Puts `count` pins at random positions on a map, each linked to a random
/// non-map entity, for the 500-pin performance check. Returns the count.
pub fn seed_map_pins(conn: &Connection, map_entity_id: &str, count: u32) -> AppResult<u32> {
    let info = db::maps::get_map_by_entity(conn, map_entity_id)?
        .ok_or_else(|| AppError::NotFound(format!("No map for entity {map_entity_id}")))?;
    let layer_id = db::maps::first_layer_id(conn, &info.id)?
        .ok_or_else(|| AppError::Internal("map has no layer".into()))?;
    let (w, h) = if info.width > 0 && info.height > 0 {
        (info.width, info.height)
    } else {
        FALLBACK_MAP_SIZE
    };

    // Link pins to any entity that is not a map.
    let mut targets = Vec::new();
    for t in TYPES {
        targets.extend(db::maps::entity_ids_of_type(conn, t)?);
    }

    let mut rng = Lcg(Uuid::now_v7().as_u128() as u64);
    let tx = conn.unchecked_transaction()?;
    for _ in 0..count {
        let entity_id = if targets.is_empty() {
            None
        } else {
            Some(targets[rng.next(targets.len())].clone())
        };
        maps::create_object(
            &tx,
            NewMapObject {
                layer_id: layer_id.clone(),
                geometry: MapGeometry::Pin {
                    x: rng.next(w as usize) as f64,
                    y: rng.next(h as usize) as f64,
                },
                style: None,
                entity_id,
            },
        )?;
    }
    tx.commit()?;
    Ok(count)
}

/// Event names are built from these two lists.
const EVENT_KINDS: [&str; 10] = [
    "Battle of",
    "Siege of",
    "Coronation at",
    "Treaty of",
    "Founding of",
    "Fall of",
    "Plague in",
    "Festival at",
    "Council of",
    "Great Flood of",
];
const EVENT_PLACES: [&str; 8] = [
    "Ashford",
    "Stormwind",
    "Dunmere",
    "Greywatch",
    "Highcrown",
    "Ironvale",
    "Ravenhold",
    "Evenfall",
];

/// Events are spread over roughly +/- 1,500 years of the default calendar.
const SEED_DAY_SPAN: usize = 540_000;

/// Creates `count` events across a few thousand years: about a third are date
/// ranges, the rest single days. Each is linked (`involved_in`) to up to three
/// random characters. Returns the count.
pub fn seed_events(conn: &Connection, count: u32) -> AppResult<u32> {
    let calendar = crate::services::calendar::get_default(conn)?;
    let characters = db::maps::entity_ids_of_type(conn, "character")?;
    let mut rng = Lcg(Uuid::now_v7().as_u128() as u64);
    let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let tx = conn.unchecked_transaction()?;

    for i in 0..count {
        let entity = Entity {
            id: Uuid::now_v7().to_string(),
            entity_type: "event".into(),
            name: format!(
                "{} {} {}",
                EVENT_KINDS[rng.next(EVENT_KINDS.len())],
                EVENT_PLACES[rng.next(EVENT_PLACES.len())],
                i
            ),
            summary: format!("An event about {}", rng.sentence(3)),
            properties: json!({}),
            created_at: timestamp.clone(),
            updated_at: timestamp.clone(),
        };
        db::entities::insert(&tx, &entity)?;

        // `usize` -> `i64` with `as` is fine here: the values are small.
        let start = rng.next(SEED_DAY_SPAN * 2) as i64 - SEED_DAY_SPAN as i64;
        let end = (rng.next(3) == 0).then(|| start + 1 + rng.next(5000) as i64);
        let importance = 1 + rng.next(5) as i32;
        db::events::insert(&tx, &entity.id, &calendar.id, start, end, importance)?;
        db::search::replace(&tx, &entity.id, &entity.name, &entity.summary, "")?;

        if !characters.is_empty() {
            for _ in 0..rng.next(4) {
                let who = &characters[rng.next(characters.len())];
                db::links::insert_link(&tx, who, &entity.id, KIND_INVOLVED_IN)?;
            }
        }
    }

    tx.commit()?;
    Ok(count)
}

/// Creates up to `count` random relationships among existing entities, for
/// graph performance tests. Each attempt picks a random catalog kind and
/// random entities of the allowed types; attempts that the rules reject
/// (duplicate, self-link) are simply skipped, so the result can be slightly
/// below `count`. Returns how many were created.
pub fn seed_relationships(conn: &Connection, count: u32) -> AppResult<u32> {
    use crate::models::relationship::{self as catalog, KIND_CUSTOM};
    use crate::services::relationships;

    // Entity ids grouped by type, for picking valid endpoints.
    let mut by_type: Vec<(&str, Vec<String>)> = Vec::new();
    for t in ["character", "location", "faction", "item", "note"] {
        by_type.push((t, db::maps::entity_ids_of_type(conn, t)?));
    }
    let pool = |allowed: &[String]| -> Vec<&String> {
        by_type
            .iter()
            .filter(|(t, _)| allowed.is_empty() || allowed.iter().any(|a| a == t))
            .flat_map(|(_, ids)| ids.iter())
            .collect()
    };

    let kinds = catalog::all();
    let mut rng = Lcg(Uuid::now_v7().as_u128() as u64);
    let tx = conn.unchecked_transaction()?;
    let mut created = 0;
    // Bounded number of attempts so a tiny world cannot loop forever.
    for _ in 0..(count as usize).saturating_mul(10) {
        if created >= count {
            break;
        }
        let kind = &kinds[rng.next(kinds.len())];
        let (sources, targets) = (pool(&kind.source_types), pool(&kind.target_types));
        if sources.is_empty() || targets.is_empty() {
            continue;
        }
        let source = sources[rng.next(sources.len())];
        let target = targets[rng.next(targets.len())];
        let label = (kind.key == KIND_CUSTOM).then(|| rng.sentence(2));
        // Rule violations (`Validation`) just mean "try another pair".
        match relationships::create(&tx, source, target, &kind.key, label) {
            Ok(_) => created += 1,
            Err(AppError::Validation(_)) => {}
            Err(e) => return Err(e),
        }
    }
    tx.commit()?;
    Ok(created)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::db::migrations::MIGRATIONS;
    use crate::services::search;

    #[test]
    fn seeds_searchable_entities() {
        let mut c = Connection::open_in_memory().unwrap();
        c.pragma_update(None, "foreign_keys", "ON").unwrap();
        MIGRATIONS.to_latest(&mut c).unwrap();

        assert_eq!(seed(&c, 200).unwrap(), 200);
        let total: i64 = c
            .query_row("SELECT count(*) FROM entities", [], |r| r.get(0))
            .unwrap();
        assert_eq!(total, 200);
        assert!(!search::search(&c, "drag", 20).unwrap().is_empty());
    }

    #[test]
    fn seeds_valid_relationships() {
        let mut c = Connection::open_in_memory().unwrap();
        c.pragma_update(None, "foreign_keys", "ON").unwrap();
        MIGRATIONS.to_latest(&mut c).unwrap();
        seed(&c, 150).unwrap();

        let made = seed_relationships(&c, 300).unwrap();
        assert!(made > 200, "created only {made}");
        let rows: i64 = c
            .query_row(
                "SELECT count(*) FROM links WHERE kind NOT IN ('mention', 'involved_in')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(rows, i64::from(made));
        // In a world without entities there is nothing to link.
        let mut empty = Connection::open_in_memory().unwrap();
        MIGRATIONS.to_latest(&mut empty).unwrap();
        assert_eq!(seed_relationships(&empty, 10).unwrap(), 0);
    }

    #[test]
    fn seeds_map_pins() {
        use crate::models::entity::NewEntity;
        use crate::services::entities;

        let mut c = Connection::open_in_memory().unwrap();
        c.pragma_update(None, "foreign_keys", "ON").unwrap();
        MIGRATIONS.to_latest(&mut c).unwrap();
        seed(&c, 20).unwrap();
        let map = entities::create(
            &c,
            NewEntity {
                entity_type: "map".into(),
                name: "Test".into(),
            },
        )
        .unwrap();
        assert_eq!(seed_map_pins(&c, &map.id, 50).unwrap(), 50);
        let data = maps::get_map(&c, std::path::Path::new("assets"), &map.id).unwrap();
        assert_eq!(data.objects.len(), 50);
        assert!(data.objects.iter().all(|o| o.entity_id.is_some()));
    }

    #[test]
    fn seeds_events_linked_to_characters() {
        let mut c = Connection::open_in_memory().unwrap();
        c.pragma_update(None, "foreign_keys", "ON").unwrap();
        MIGRATIONS.to_latest(&mut c).unwrap();
        seed(&c, 50).unwrap();

        assert_eq!(seed_events(&c, 100).unwrap(), 100);
        let all = crate::services::events::list(&c, -1_000_000, 1_000_000).unwrap();
        assert_eq!(all.len(), 100);
        assert!(all.iter().any(|e| e.end_day.is_some()));
        assert!(all.iter().any(|e| e.start_day < 0));
        assert!(
            all.iter()
                .all(|e| e.end_day.is_none_or(|end| end > e.start_day))
        );
        let linked: i64 = c
            .query_row(
                "SELECT count(*) FROM links WHERE kind = 'involved_in'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(linked > 0);
        assert!(
            !search::search(&c, "battle", 5).unwrap().is_empty()
                || !search::search(&c, "siege", 5).unwrap().is_empty()
        );
    }
}
