//! Links between entities. For now only `mention` links, which the lore
//! editor creates with `[[` and which this module keeps in sync with the
//! saved document.

use rusqlite::Connection;
use serde_json::Value;

use crate::db;
use crate::error::AppResult;
use crate::models::link::{Backlink, KIND_MENTION};

/// Walks TipTap JSON and collects the entity ids of every `mention` node,
/// in document order and without duplicates.
pub fn extract_mention_ids(content: &Value) -> Vec<String> {
    let mut ids = Vec::new();
    walk(content, &mut ids);
    ids
}

fn walk(node: &Value, ids: &mut Vec<String>) {
    if node.get("type").and_then(Value::as_str) == Some("mention")
        && let Some(id) = node.pointer("/attrs/id").and_then(Value::as_str)
        && !ids.iter().any(|existing| existing == id)
    {
        ids.push(id.to_owned());
    }
    if let Some(children) = node.get("content").and_then(Value::as_array) {
        for child in children {
            walk(child, ids);
        }
    }
}

/// Makes the `mention` links of `source_id` match what `content` mentions.
pub fn sync_mentions(conn: &Connection, source_id: &str, content: &Value) -> AppResult<()> {
    let targets = extract_mention_ids(content);
    db::links::replace_from_source(conn, source_id, KIND_MENTION, &targets)
}

/// Entities whose lore mentions `target_id`, each with a short snippet of
/// text around the first occurrence of `target_name`.
pub fn backlinks(
    conn: &Connection,
    target_id: &str,
    target_name: &str,
) -> AppResult<Vec<Backlink>> {
    let rows = db::links::sources_for_target(conn, target_id, KIND_MENTION)?;
    Ok(rows
        .into_iter()
        .map(|row| Backlink {
            snippet: snippet(&row.plain_text, target_name),
            source_id: row.source_id,
            source_type: row.source_type,
            source_name: row.source_name,
        })
        .collect())
}

const SNIPPET_CHARS: usize = 70;

/// Up to ~140 characters of `text` centred on `needle` (case-insensitive),
/// or the start of the text when the name is not found.
pub fn snippet(text: &str, needle: &str) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    // Work in chars, not bytes: slicing a String in the middle of a multi-byte
    // character would panic.
    let chars: Vec<char> = text.chars().collect();
    let lower: Vec<char> = text.to_lowercase().chars().collect();
    let needle: Vec<char> = needle.to_lowercase().chars().collect();

    let hit = (!needle.is_empty())
        .then(|| {
            lower
                .windows(needle.len())
                .position(|w| w == needle.as_slice())
        })
        .flatten();
    let (start, end) = match hit {
        Some(pos) => (
            pos.saturating_sub(SNIPPET_CHARS),
            (pos + needle.len() + SNIPPET_CHARS).min(chars.len()),
        ),
        None => (0, (SNIPPET_CHARS * 2).min(chars.len())),
    };
    let mut out = String::new();
    if start > 0 {
        out.push('…');
    }
    out.extend(&chars[start..end]);
    if end < chars.len() {
        out.push('…');
    }
    out
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::db::migrations::MIGRATIONS;
    use crate::models::entity::NewEntity;
    use crate::services::{documents, entities};
    use serde_json::json;

    fn mention(id: &str) -> Value {
        json!({ "type": "mention", "attrs": { "id": id } })
    }

    fn doc_with(mentions: &[&str]) -> Value {
        let inline: Vec<Value> = mentions.iter().map(|m| mention(m)).collect();
        json!({ "type": "doc", "content": [
            { "type": "paragraph", "content": [{ "type": "text", "text": "See " }] },
            { "type": "blockquote", "content": [{ "type": "paragraph", "content": inline }] }
        ]})
    }

    #[test]
    fn extracts_nested_mentions_without_duplicates() {
        let ids = extract_mention_ids(&doc_with(&["b", "a", "b"]));
        assert_eq!(ids, vec!["b", "a"]);
        assert!(extract_mention_ids(&json!({ "type": "doc", "content": [] })).is_empty());
    }

    fn world() -> (Connection, String, String) {
        let mut c = Connection::open_in_memory().unwrap();
        c.pragma_update(None, "foreign_keys", "ON").unwrap();
        MIGRATIONS.to_latest(&mut c).unwrap();
        let new = |t: &str, n: &str| NewEntity {
            entity_type: t.into(),
            name: n.into(),
        };
        let ada = entities::create(&c, new("character", "Ada")).unwrap();
        let town = entities::create(&c, new("location", "Rivertown")).unwrap();
        (c, ada.id, town.id)
    }

    #[test]
    fn saving_a_document_adds_and_removes_mention_links() {
        let (c, ada, town) = world();

        documents::save(
            &c,
            &ada,
            doc_with(&[&town]).to_string(),
            "Born in Rivertown".into(),
        )
        .unwrap();
        assert_eq!(
            db::links::targets_for_source(&c, &ada, KIND_MENTION).unwrap(),
            vec![town.clone()]
        );

        let links = backlinks(&c, &town, "Rivertown").unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].source_name, "Ada");
        assert!(links[0].snippet.contains("Rivertown"));

        // Removing the mention removes the link; a dangling id is ignored.
        documents::save(
            &c,
            &ada,
            doc_with(&["does-not-exist"]).to_string(),
            String::new(),
        )
        .unwrap();
        assert!(
            db::links::targets_for_source(&c, &ada, KIND_MENTION)
                .unwrap()
                .is_empty()
        );
        assert!(backlinks(&c, &town, "Rivertown").unwrap().is_empty());
    }

    #[test]
    fn deleting_the_target_removes_its_links() {
        let (c, ada, town) = world();
        documents::save(&c, &ada, doc_with(&[&town]).to_string(), String::new()).unwrap();
        entities::delete(&c, &town).unwrap();
        assert!(
            db::links::targets_for_source(&c, &ada, KIND_MENTION)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn snippet_centres_on_the_name_and_handles_unicode() {
        let long = format!("{} Rivertown {}", "a ".repeat(100), "b ".repeat(100));
        let s = snippet(&long, "rivertown");
        assert!(s.starts_with('…') && s.ends_with('…'));
        assert!(s.contains("Rivertown"));
        assert!(s.chars().count() <= SNIPPET_CHARS * 2 + 9 + 2);

        assert_eq!(snippet("short text", "missing"), "short text");
        let s = snippet("日本語のテキスト Rivertown です", "Rivertown");
        assert!(s.contains("Rivertown"));
    }
}
