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
use crate::services::tags;

/// Upper bound for `limit`, so a buggy caller cannot ask for everything.
const MAX_LIMIT: u32 = 200;

/// Re-reads the entity (and its document and tags, if any) and rewrites its
/// index row. Call this inside the transaction of any write that changes
/// name, summary, tags or lore. If the entity no longer exists, its index row
/// is removed.
pub fn reindex_entity(conn: &Connection, entity_id: &str) -> AppResult<()> {
    let Some(entity) = db::entities::get(conn, entity_id)? else {
        return db::search::delete(conn, entity_id);
    };
    let body = db::documents::get(conn, entity_id)?
        .map(|d| d.plain_text)
        .unwrap_or_default();
    // `entity.tags` is already sorted; the FTS column is just the names
    // separated by spaces ("places/cities" is split into words by the tokenizer).
    db::search::replace(
        conn,
        &entity.id,
        &entity.name,
        &entity.summary,
        &entity.tags.join(" "),
        &body,
    )
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

/// A search box query split into its two kinds of parts.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ParsedQuery {
    /// The free text (everything except the `tag:` filters).
    pub text: String,
    /// The tag names from `tag:name` / `tag:"name with spaces"` filters.
    pub tags: Vec<String>,
}

/// Splits `tag:` filters out of a search query.
///
/// Syntax: `tag:royalty`, `tag:places/cities` or, for names with spaces,
/// `tag:"dark age"`. `tag:` is case-insensitive and only counts at the start
/// of a word. Every `tag:` filter must match (AND), and a tag also matches
/// its nested tags (`tag:places` finds `places/cities`). Everything else is
/// the free text, searched as before. A bare `tag:` is plain text.
pub fn parse_query(query: &str) -> ParsedQuery {
    let chars: Vec<char> = query.chars().collect();
    let mut parsed = ParsedQuery::default();
    let mut text = String::new();
    let mut i = 0;
    while i < chars.len() {
        let at_word_start = i == 0 || chars[i - 1].is_whitespace();
        let is_filter = at_word_start
            && chars.len() >= i + 4
            && chars[i..i + 4]
                .iter()
                .collect::<String>()
                .eq_ignore_ascii_case("tag:");
        if !is_filter {
            text.push(chars[i]);
            i += 1;
            continue;
        }
        let mut j = i + 4;
        let mut name = String::new();
        if chars.get(j) == Some(&'"') {
            j += 1;
            while j < chars.len() && chars[j] != '"' {
                name.push(chars[j]);
                j += 1;
            }
            j += 1; // the closing quote (or past the end)
        } else {
            while j < chars.len() && !chars[j].is_whitespace() {
                name.push(chars[j]);
                j += 1;
            }
        }
        if name.trim().trim_start_matches('#').is_empty() {
            // Nothing after "tag:": keep it as ordinary text.
            text.extend(&chars[i..j.min(chars.len())]);
        } else {
            parsed.tags.push(name);
            text.push(' ');
        }
        i = j;
    }
    parsed.text = text;
    parsed
}

/// Searches names, summaries, tags and lore. `tag:` filters (see
/// [`parse_query`]) narrow the results; with only filters the matching
/// entities are listed by type and name. Empty queries return no results.
pub fn search(conn: &Connection, query: &str, limit: u32) -> AppResult<Vec<SearchResult>> {
    let parsed = parse_query(query);
    let expr = build_match_expression(&parsed.text);
    if expr.is_none() && parsed.tags.is_empty() {
        return Ok(Vec::new());
    }
    let mut groups: Vec<Vec<String>> = Vec::new();
    if !parsed.tags.is_empty() {
        let all = db::tags::list(conn)?;
        for name in &parsed.tags {
            let group = tags::tag_group(&all, name);
            if group.is_empty() {
                // A filter for a tag that does not exist matches nothing.
                return Ok(Vec::new());
            }
            groups.push(group);
        }
    }
    db::search::query(conn, expr.as_deref(), &groups, limit.clamp(1, MAX_LIMIT))
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
    use crate::services::{documents, entities, tags};

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

    fn tag(c: &Connection, id: &str, names: &[&str]) {
        let names: Vec<String> = names.iter().map(|n| (*n).to_owned()).collect();
        tags::set_entity_tags(c, id, &names).unwrap();
    }

    #[test]
    fn parse_query_splits_tag_filters_from_text() {
        let p = parse_query("dragon tag:royalty TAG:\"dark age\" lair");
        assert_eq!(p.tags, vec!["royalty", "dark age"]);
        assert_eq!(
            p.text.split_whitespace().collect::<Vec<_>>(),
            vec!["dragon", "lair"]
        );
        // `tag:` only counts at the start of a word; a bare `tag:` is text.
        assert!(parse_query("notag:x").tags.is_empty());
        let bare = parse_query("tag: foo");
        assert!(bare.tags.is_empty());
        assert!(bare.text.contains("tag:"));
        assert_eq!(parse_query("tag:places/cities").tags, vec!["places/cities"]);
        assert_eq!(parse_query("tag:#royal").tags, vec!["#royal"]);
        assert_eq!(
            parse_query("tag:\"unclosed name").tags,
            vec!["unclosed name"]
        );
    }

    #[test]
    fn tag_filter_narrows_text_search_and_alone_lists_by_tag() {
        let c = conn();
        let queen = make(&c, "character", "Queen Isolde");
        let king = make(&c, "character", "King Aldous");
        let city = make(&c, "location", "Port Veyra");
        set_lore(&c, &queen, "She rules the coast");
        set_lore(&c, &king, "He ruled the coast");
        tag(&c, &queen, &["royalty", "coast"]);
        tag(&c, &king, &["royalty"]);
        tag(&c, &city, &["places/cities", "coast"]);

        assert_eq!(ids(&c, "coast").len(), 3); // the tag text itself is searchable
        assert_eq!(ids(&c, "rule tag:royalty").len(), 2);
        assert_eq!(ids(&c, "rule tag:royalty tag:coast"), vec![queen.clone()]);
        // Only filters: everything with the tag, ordered by type then name.
        assert_eq!(ids(&c, "tag:royalty"), vec![king.clone(), queen.clone()]);
        // Nested: `places` finds `places/cities`; case does not matter.
        assert_eq!(ids(&c, "Tag:PLACES"), vec![city.clone()]);
        assert_eq!(ids(&c, "tag:places/cities veyra"), vec![city]);
        // Unknown tag: nothing. A filter never errors.
        assert!(ids(&c, "tag:nope").is_empty());
        assert!(ids(&c, "queen tag:nope").is_empty());
        assert!(search(&c, "tag:\"", 5).is_ok());
        // Tag-only hits show the summary as the snippet.
        entities::update(
            &c,
            &queen,
            EntityPatch {
                summary: Some("Young ruler".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let hits = search(&c, "tag:coast", 10).unwrap();
        assert!(hits.iter().any(|h| h.snippet == "Young ruler"));
    }

    #[test]
    fn tags_are_searchable_text_and_follow_renames_and_deletes() {
        let c = conn();
        let id = make(&c, "note", "Plain");
        tag(&c, &id, &["gloomy"]);
        assert_eq!(ids(&c, "gloomy"), vec![id.clone()]);
        let t = tags::list(&c).unwrap().remove(0);
        tags::rename(&c, &t.id, "cheerful").unwrap();
        assert!(ids(&c, "gloomy").is_empty());
        assert_eq!(ids(&c, "cheerful"), vec![id.clone()]);
        tags::delete(&c, &t.id).unwrap();
        assert!(ids(&c, "cheerful").is_empty());
        // Rebuilding the index keeps tags.
        tag(&c, &id, &["places/cities"]);
        c.execute("DELETE FROM search_index", []).unwrap();
        rebuild(&c).unwrap();
        assert_eq!(ids(&c, "cities"), vec![id]);
    }

    #[test]
    fn migration_012_recreates_the_index_and_keeps_data() {
        let mut c = Connection::open_in_memory().unwrap();
        MIGRATIONS.to_version(&mut c, 11).unwrap();
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
        // The new column exists and works.
        c.pragma_update(None, "foreign_keys", "ON").unwrap();
        tag(&c, "1", &["royal"]);
        assert_eq!(ids(&c, "royal"), vec!["1".to_string()]);
        let favorite: i64 = c
            .query_row("SELECT favorite FROM entities", [], |r| r.get(0))
            .unwrap();
        assert_eq!(favorite, 0);
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
