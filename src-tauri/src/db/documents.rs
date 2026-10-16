//! Repository functions for the `documents` table.

use rusqlite::{Connection, OptionalExtension, params};

use crate::error::AppResult;
use crate::models::document::Document;

pub fn get(conn: &Connection, entity_id: &str) -> AppResult<Option<Document>> {
    let doc = conn
        .query_row(
            "SELECT entity_id, content, plain_text, updated_at FROM documents WHERE entity_id = ?1",
            params![entity_id],
            |row| {
                Ok(Document {
                    entity_id: row.get("entity_id")?,
                    content: row.get("content")?,
                    plain_text: row.get("plain_text")?,
                    updated_at: row.get("updated_at")?,
                })
            },
        )
        .optional()?;
    Ok(doc)
}

/// Inserts or replaces the document for an entity ("upsert").
pub fn upsert(conn: &Connection, doc: &Document) -> AppResult<()> {
    conn.execute(
        "INSERT INTO documents (entity_id, content, plain_text, updated_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(entity_id) DO UPDATE SET
            content = excluded.content,
            plain_text = excluded.plain_text,
            updated_at = excluded.updated_at",
        params![doc.entity_id, doc.content, doc.plain_text, doc.updated_at],
    )?;
    Ok(())
}
