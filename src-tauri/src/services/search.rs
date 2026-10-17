//! Full-text search.
//!
//! Index sync strategy: the SERVICE layer keeps `search_index` up to date
//! (not SQL triggers), inside the same transaction as each entity/document
//! write. Reason: the lore text arrives from the editor as `plain_text`, and
//! the service layer is where we already coordinate several tables (e.g.
//! mention links). Triggers would be invisible and harder to test.
//! `rebuild` repairs the index if it ever drifts.

use rusqlite::Connection;

use crate::db;
use crate::error::AppResult;
use crate::models::search::SearchResult;

/// Upper bound for `limit`, so a buggy caller cannot ask for everything.
const MAX_LIMIT: u32 = 200;

/// Re-reads the entity (and its document, if any) and rewrites its index row.
/// Call this inside the transaction of any write that changes name, summary
/// or lore. If the entity no longer exists, its index row is removed.
pub fn reindex_entity(conn: &Connection, entity_id: &str) -> AppResult<()> {
    let Some(entity) = db::entities::get(conn, entity_id)? else {
        return db::search::delete(conn, entity_id);
    };
    let body = db::documents::get(conn, entity_id)?
        .map(|d| d.plain_text)
        .unwrap_or_default();
    db::search::replace(conn, &entity.id, &entity.name, &entity.summary, &body)
}

pub fn remove_entity(conn: &Connection, entity_id: &str) -> AppResult<()> {
    db::search::delete(conn, entity_id)
}

/// Turns free user text into a safe FTS5 expression, or `None` if there is
/// nothing searchable.
///
/// FTS5 has its own query syntax (`"`, `*`, `-`, `:`, `^`, `AND`, `NEAR`...),
/// and invalid syntax is a SQL error. We avoid all of it: split the input
/// on anything that is not a letter or digit (the same rule the tokenizer
/// uses), wrap each word in double quotes (so keywords like `OR` are plain
/// words), add `*` for prefix matching, and join with spaces (implicit AND).
pub fn build_match_expression(query: &str) -> Option<String> {
    let terms: Vec<String> = query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{t}\"*"))
        .collect();
    if terms.is_empty() {
        None
    } else {
        Some(terms.join(" "))
    }
}

/// Searches names, summaries and lore. Empty queries return no results.
pub fn search(conn: &Connection, query: &str, limit: u32) -> AppResult<Vec<SearchResult>> {
    let Some(expr) = build_match_expression(query) else {
        return Ok(Vec::new());
    };
    db::search::query(conn, &expr, limit.clamp(1, MAX_LIMIT))
}

/// Clears and repopulates the whole index in one transaction.
pub fn rebuild(conn: &Connection) -> AppResult<()> {
    let tx = conn.unchecked_transaction()?;
    db::search::rebuild_all(&tx)?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::db::migrations::MIGRATIONS;
    use crate::models::entity::{EntityPatch, NewEntity};
    use crate::models::search::{HIGHLIGHT_END, HIGHLIGHT_START};
    use crate::services::{documents, entities};

    fn conn() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        c.pragma_update(None, "foreign_keys", "ON").unwrap();
        MIGRATIONS.to_latest(&mut c).unwrap();
        c
    }

    fn make(c: &Connection, entity_type: &str, name: &str) -> String {
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

    fn set_lore(c: &Connection, id: &str, text: &str) {
        documents::save(c, id, r#"{"type":"doc","content":[]}"#.into(), text.into()).unwrap();
    }

    fn ids(c: &Connection, q: &str) -> Vec<String> {
        search(c, q, 50)
            .unwrap()
            .into_iter()
            .map(|r| r.entity_id)
            .collect()
    }

    #[test]
    fn indexes_on_create_update_and_delete() {
        let c = conn();
        let id = make(&c, "character", "Aldric");
        assert_eq!(ids(&c, "aldric"), vec![id.clone()]);

        entities::update(
            &c,
            &id,
            EntityPatch {
                name: Some("Borin".into()),
                summary: Some("A stoic dwarf smith".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(ids(&c, "aldric").is_empty());
        assert_eq!(ids(&c, "borin"), vec![id.clone()]);
        assert_eq!(ids(&c, "dwarf"), vec![id.clone()]);

        entities::delete(&c, &id).unwrap();
        assert!(ids(&c, "borin").is_empty());
    }

    #[test]
    fn indexes_document_saves_and_replaces_old_text() {
        let c = conn();
        let id = make(&c, "location", "Rivertown");
        set_lore(&c, &id, "Founded beside the silver river");
        assert_eq!(ids(&c, "silver"), vec![id.clone()]);

        set_lore(&c, &id, "Now ruled by merchants");
        assert!(ids(&c, "silver").is_empty());
        assert_eq!(ids(&c, "merchants"), vec![id.clone()]);

        // Renaming keeps the lore searchable (one row per entity).
        entities::update(
            &c,
            &id,
            EntityPatch {
                name: Some("Rivergate".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(ids(&c, "merchants"), vec![id.clone()]);
        let rows: i64 = c
            .query_row("SELECT count(*) FROM search_index", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 1);
    }

    #[test]
    fn prefix_and_multi_word_search() {
        let c = conn();
        let id = make(&c, "character", "Elowen Brightwater");
        make(&c, "character", "Elric");
        assert_eq!(ids(&c, "elo"), vec![id.clone()]);
        assert_eq!(ids(&c, "elowen bright"), vec![id.clone()]);
        assert_eq!(ids(&c, "el").len(), 2);
        assert!(ids(&c, "elowen zzz").is_empty());
    }

    #[test]
    fn snippet_contains_highlight_markers() {
        let c = conn();
        let id = make(&c, "note", "Prophecy");
        set_lore(&c, &id, "The dragon sleeps beneath the mountain");
        let hits = search(&c, "drag", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].name, "Prophecy");
        assert_eq!(hits[0].entity_type, "note");
        let expected = format!("{HIGHLIGHT_START}dragon{HIGHLIGHT_END}");
        assert!(hits[0].snippet.contains(&expected), "{:?}", hits[0].snippet);
    }

    #[test]
    fn special_characters_never_error() {
        let c = conn();
        let id = make(&c, "character", "Dragon's \"Bane\"");
        set_lore(&c, &id, "AND OR NOT near plain");
        let nasty = [
            "\"",
            "\"\"\"",
            "(",
            ")",
            "(((",
            "*",
            "**",
            "-",
            "- -x",
            ":",
            "name:foo",
            "^bar",
            "NEAR",
            "NEAR(a b)",
            "AND",
            "OR",
            "NOT",
            "a AND",
            "x OR y",
            "'; DROP TABLE entities; --",
            "dragon's",
            "{col}:x",
            "\u{2}\u{3}",
            "🐉",
            "日本語",
            "   ",
            "",
        ];
        for q in nasty {
            assert!(search(&c, q, 10).is_ok(), "query {q:?} failed");
        }
        assert!(search(&c, "", 10).unwrap().is_empty());
        assert!(search(&c, "   ", 10).unwrap().is_empty());
        // Keywords are treated as ordinary words.
        assert_eq!(ids(&c, "and or"), vec![id.clone()]);
        assert_eq!(ids(&c, "dragon's bane"), vec![id]);
    }

    #[test]
    fn name_match_outranks_lore_only_match() {
        let c = conn();
        let lore_only = make(&c, "note", "Chronicle");
        set_lore(&c, &lore_only, "Many tales mention the Wyvern here");
        let by_name = make(&c, "character", "Wyvern");
        // Insert the lore match first so ordering is not just insertion order.
        let hits = ids(&c, "wyvern");
        assert_eq!(hits, vec![by_name, lore_only]);
    }

    #[test]
    fn diacritics_are_ignored() {
        let c = conn();
        let id = make(&c, "character", "Élan");
        assert_eq!(ids(&c, "elan"), vec![id.clone()]);
        assert_eq!(ids(&c, "Élan"), vec![id]);
    }

    #[test]
    fn rebuild_restores_a_damaged_index() {
        let c = conn();
        let id = make(&c, "faction", "Silver Guild");
        set_lore(&c, &id, "Traders of the north");
        c.execute("DELETE FROM search_index", []).unwrap();
        assert!(ids(&c, "guild").is_empty());

        rebuild(&c).unwrap();
        assert_eq!(ids(&c, "guild"), vec![id.clone()]);
        assert_eq!(ids(&c, "traders"), vec![id]);
    }

    #[test]
    fn migration_backfills_existing_rows() {
        let mut c = Connection::open_in_memory().unwrap();
        // Migrate to 004 only, add data, then apply 005.
        MIGRATIONS
            .to_version(&mut c, 4)
            .expect("migrate to version 4");
        c.execute(
            "INSERT INTO entities (id, type, name, summary, properties, created_at, updated_at)
             VALUES ('1', 'note', 'Old Note', 'legacy summary', '{}', 't', 't')",
            [],
        )
        .unwrap();
        c.execute(
            "INSERT INTO documents (entity_id, content, plain_text, updated_at)
             VALUES ('1', '{}', 'ancient lore', 't')",
            [],
        )
        .unwrap();
        MIGRATIONS.to_latest(&mut c).unwrap();
        assert_eq!(ids(&c, "legacy"), vec!["1".to_string()]);
        assert_eq!(ids(&c, "ancient"), vec!["1".to_string()]);
    }
}
