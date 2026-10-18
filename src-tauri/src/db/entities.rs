//! Repository functions for the `entities` table. Only this file writes SQL
//! for entities; services call these with an open `Connection`.

use rusqlite::{Connection, OptionalExtension, Row, params};
use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::models::entity::{Entity, EntitySummary};

/// Maps one row (all columns) to an `Entity`. `Row::get` is typed by the
/// variable it's assigned to, e.g. `let name: String = row.get("name")?`.
fn map_entity(row: &Row<'_>) -> rusqlite::Result<Entity> {
    let properties: String = row.get("properties")?;
    Ok(Entity {
        id: row.get("id")?,
        entity_type: row.get("type")?,
        name: row.get("name")?,
        summary: row.get("summary")?,
        // Stored as text; parse it here so callers see a JSON value. A corrupt
        // value becomes an empty object rather than breaking the whole list.
        properties: serde_json::from_str(&properties).unwrap_or(Value::Object(Default::default())),
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn map_summary(row: &Row<'_>) -> rusqlite::Result<EntitySummary> {
    Ok(EntitySummary {
        id: row.get("id")?,
        entity_type: row.get("type")?,
        name: row.get("name")?,
        updated_at: row.get("updated_at")?,
    })
}

pub fn insert(conn: &Connection, entity: &Entity) -> AppResult<()> {
    conn.execute(
        "INSERT INTO entities (id, type, name, summary, properties, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            entity.id,
            entity.entity_type,
            entity.name,
            entity.summary,
            entity.properties.to_string(),
            entity.created_at,
            entity.updated_at,
        ],
    )?;
    Ok(())
}

/// `Option` because "not found" is a normal outcome for a lookup; the
/// service decides whether that is an error.
pub fn get(conn: &Connection, id: &str) -> AppResult<Option<Entity>> {
    let entity = conn
        .query_row(
            "SELECT * FROM entities WHERE id = ?1",
            params![id],
            map_entity,
        )
        .optional()?;
    Ok(entity)
}

/// All entities (optionally of one type), sorted by type then name.
pub fn list(conn: &Connection, entity_type: Option<&str>) -> AppResult<Vec<EntitySummary>> {
    let mut stmt = conn.prepare(
        "SELECT id, type, name, updated_at FROM entities
         WHERE (?1 IS NULL OR type = ?1)
         ORDER BY type, name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map(params![entity_type], map_summary)?;
    // `collect` into `Result<Vec<_>, _>` stops at the first failing row.
    let items = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(items)
}

/// Overwrites name, summary, properties and updated_at of an existing row.
pub fn update(conn: &Connection, entity: &Entity) -> AppResult<()> {
    let changed = conn.execute(
        "UPDATE entities SET name = ?2, summary = ?3, properties = ?4, updated_at = ?5
         WHERE id = ?1",
        params![
            entity.id,
            entity.name,
            entity.summary,
            entity.properties.to_string(),
            entity.updated_at,
        ],
    )?;
    if changed == 0 {
        return Err(AppError::NotFound(format!(
            "Entity {} does not exist",
            entity.id
        )));
    }
    Ok(())
}

pub fn delete(conn: &Connection, id: &str) -> AppResult<()> {
    let changed = conn.execute("DELETE FROM entities WHERE id = ?1", params![id])?;
    if changed == 0 {
        return Err(AppError::NotFound(format!("Entity {id} does not exist")));
    }
    Ok(())
}

/// Largest number of `?` placeholders used in one `IN (...)` list. SQLite's
/// own limit is far higher on modern versions, but staying small is safe.
pub const IN_CHUNK: usize = 300;

/// `?,?,?` with `n` placeholders, for building `IN (...)` lists.
pub fn placeholders(n: usize) -> String {
    vec!["?"; n].join(",")
}

/// The entities with the given ids (unknown ids are skipped). Works in
/// chunks so a long id list never exceeds SQLite's parameter limit.
pub fn summaries_by_ids(conn: &Connection, ids: &[String]) -> AppResult<Vec<EntitySummary>> {
    let mut out = Vec::new();
    for chunk in ids.chunks(IN_CHUNK) {
        let sql = format!(
            "SELECT id, type, name, updated_at FROM entities WHERE id IN ({})",
            placeholders(chunk.len())
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(chunk.iter()), map_summary)?;
        for row in rows {
            out.push(row?);
        }
    }
    Ok(out)
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

    fn entity(id: &str, entity_type: &str, name: &str) -> Entity {
        Entity {
            id: id.into(),
            entity_type: entity_type.into(),
            name: name.into(),
            summary: String::new(),
            properties: json!({}),
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn crud_round_trip() {
        let c = conn();
        let mut e = entity("1", "character", "Ada");
        e.properties = json!({ "species": "human", "age": 31 });
        insert(&c, &e).unwrap();

        let got = get(&c, "1").unwrap().unwrap();
        assert_eq!(got, e); // properties JSON survives the text round trip

        e.name = "Ada Lovelace".into();
        e.summary = "Mathematician".into();
        update(&c, &e).unwrap();
        assert_eq!(get(&c, "1").unwrap().unwrap().name, "Ada Lovelace");

        delete(&c, "1").unwrap();
        assert!(get(&c, "1").unwrap().is_none());
    }

    #[test]
    fn list_filters_by_type_and_sorts_by_name() {
        let c = conn();
        insert(&c, &entity("1", "location", "Zanzibar")).unwrap();
        insert(&c, &entity("2", "character", "bob")).unwrap();
        insert(&c, &entity("3", "character", "Alice")).unwrap();

        let all = list(&c, None).unwrap();
        let names: Vec<&str> = all.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["Alice", "bob", "Zanzibar"]);

        let chars = list(&c, Some("character")).unwrap();
        assert_eq!(chars.len(), 2);
    }

    #[test]
    fn update_and_delete_missing_rows_report_not_found() {
        let c = conn();
        assert!(matches!(delete(&c, "nope"), Err(AppError::NotFound(_))));
        assert!(matches!(
            update(&c, &entity("nope", "note", "x")),
            Err(AppError::NotFound(_))
        ));
    }
}
