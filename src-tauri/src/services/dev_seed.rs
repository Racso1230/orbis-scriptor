//! Development helper that fills a world with generated entities so search
//! can be tried with ~1,000 rows. Only reachable from the debug-only guard in
//! `commands::search::dev_seed_entities`.

use chrono::{SecondsFormat, Utc};
use rusqlite::Connection;
use serde_json::json;
use uuid::Uuid;

use crate::db;
use crate::error::AppResult;
use crate::models::document::Document;
use crate::models::entity::Entity;

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
}
