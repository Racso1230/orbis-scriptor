//! Lore documents: one rich-text document per entity.

use chrono::{SecondsFormat, Utc};
use rusqlite::Connection;

use crate::db;
use crate::error::{AppError, AppResult};
use crate::models::document::Document;
use crate::services::{links, search};

/// An empty TipTap document, returned when an entity has no lore yet so the
/// frontend never has to special-case "no row".
pub const EMPTY_CONTENT: &str = r#"{"type":"doc","content":[]}"#;

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn ensure_entity_exists(conn: &Connection, entity_id: &str) -> AppResult<()> {
    if db::entities::get(conn, entity_id)?.is_none() {
        return Err(AppError::NotFound(format!(
            "Entity {entity_id} does not exist"
        )));
    }
    Ok(())
}

pub fn get(conn: &Connection, entity_id: &str) -> AppResult<Document> {
    ensure_entity_exists(conn, entity_id)?;
    Ok(
        db::documents::get(conn, entity_id)?.unwrap_or_else(|| Document {
            entity_id: entity_id.to_owned(),
            content: EMPTY_CONTENT.to_owned(),
            plain_text: String::new(),
            updated_at: String::new(),
        }),
    )
}

pub fn save(
    conn: &Connection,
    entity_id: &str,
    content: String,
    plain_text: String,
) -> AppResult<Document> {
    ensure_entity_exists(conn, entity_id)?;
    // We don't interpret the JSON, but we refuse to store something that
    // isn't JSON at all so a bug can't corrupt a world silently.
    let parsed: serde_json::Value = serde_json::from_str(&content)
        .map_err(|_| AppError::Validation("Document content must be JSON".into()))?;
    let doc = Document {
        entity_id: entity_id.to_owned(),
        content,
        plain_text,
        updated_at: now(),
    };
    // The document, its mention links and its search entry change together or not at all.
    // `unchecked_transaction` works with a shared `&Connection`; it is "unchecked"
    // only in that the compiler can't stop us nesting transactions.
    let tx = conn.unchecked_transaction()?;
    write_doc(&tx, &doc, &parsed)?;
    tx.commit()?;
    Ok(doc)
}

/// Like [`save`] but WITHOUT its own transaction: the caller must already
/// be inside one (the world generator saves its lore stubs together with
/// everything else).
pub fn save_in(
    conn: &Connection,
    entity_id: &str,
    content: String,
    plain_text: String,
) -> AppResult<Document> {
    ensure_entity_exists(conn, entity_id)?;
    let parsed: serde_json::Value = serde_json::from_str(&content)
        .map_err(|_| AppError::Validation("Document content must be JSON".into()))?;
    let doc = Document {
        entity_id: entity_id.to_owned(),
        content,
        plain_text,
        updated_at: now(),
    };
    write_doc(conn, &doc, &parsed)?;
    Ok(doc)
}

/// The rows behind [`save`] and [`save_in`].
fn write_doc(conn: &Connection, doc: &Document, parsed: &serde_json::Value) -> AppResult<()> {
    db::documents::upsert(conn, doc)?;
    // Saving lore is an edit of the entity ("recently edited" list).
    db::entities::touch(conn, &doc.entity_id, &doc.updated_at)?;
    links::sync_mentions(conn, &doc.entity_id, parsed)?;
    search::reindex_entity(conn, &doc.entity_id)?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::db::migrations::MIGRATIONS;
    use crate::models::entity::NewEntity;
    use crate::services::entities;

    fn world() -> (Connection, String) {
        let mut c = Connection::open_in_memory().unwrap();
        c.pragma_update(None, "foreign_keys", "ON").unwrap();
        MIGRATIONS.to_latest(&mut c).unwrap();
        let e = entities::create(
            &c,
            NewEntity {
                entity_type: "character".into(),
                name: "Ada".into(),
            },
        )
        .unwrap();
        (c, e.id)
    }

    #[test]
    fn missing_document_reads_as_empty() {
        let (c, id) = world();
        let doc = get(&c, &id).unwrap();
        assert_eq!(doc.content, EMPTY_CONTENT);
        assert_eq!(doc.plain_text, "");
    }

    #[test]
    fn save_then_get_round_trip_and_overwrite() {
        let (c, id) = world();
        let content = r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Hi"}]}]}"#;
        let saved = save(&c, &id, content.into(), "Hi".into()).unwrap();
        assert_eq!(get(&c, &id).unwrap(), saved);

        let again = save(&c, &id, EMPTY_CONTENT.into(), String::new()).unwrap();
        assert_eq!(get(&c, &id).unwrap().content, EMPTY_CONTENT);
        assert!(again.updated_at >= saved.updated_at);
    }

    #[test]
    fn rejects_unknown_entity_and_non_json() {
        let (c, id) = world();
        assert!(matches!(get(&c, "nope"), Err(AppError::NotFound(_))));
        assert!(matches!(
            save(&c, &id, "not json".into(), String::new()),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn deleting_the_entity_cascades_to_its_document() {
        let (c, id) = world();
        save(&c, &id, EMPTY_CONTENT.into(), String::new()).unwrap();
        entities::delete(&c, &id).unwrap();
        assert!(db::documents::get(&c, &id).unwrap().is_none());
    }
}
