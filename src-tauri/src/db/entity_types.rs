//! Repository functions for `custom_entity_types` and `type_templates`
//! (user-defined entity types and extra fields), plus the few `entities`
//! statements that only matter when a type changes.

use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::error::AppResult;
use crate::models::entity_type::{EntityTypeDef, FieldDef};

/// Field lists are stored as JSON text. A corrupt value reads as "no fields"
/// instead of making every type unusable.
fn parse_fields(text: &str) -> Vec<FieldDef> {
    serde_json::from_str(text).unwrap_or_default()
}

fn map_type(row: &Row<'_>) -> rusqlite::Result<EntityTypeDef> {
    let fields: String = row.get("fields")?;
    Ok(EntityTypeDef {
        key: row.get("key")?,
        label: row.get("label")?,
        plural: row.get("plural")?,
        icon: row.get("icon")?,
        color: row.get("color")?,
        builtin: false,
        fields: parse_fields(&fields),
    })
}

/// All custom types in their display order.
pub fn list_custom(conn: &Connection) -> AppResult<Vec<EntityTypeDef>> {
    let mut stmt =
        conn.prepare("SELECT * FROM custom_entity_types ORDER BY sort_order, created_at, key")?;
    let rows = stmt.query_map([], map_type)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn get_custom(conn: &Connection, key: &str) -> AppResult<Option<EntityTypeDef>> {
    Ok(conn
        .query_row(
            "SELECT * FROM custom_entity_types WHERE key = ?1",
            params![key],
            map_type,
        )
        .optional()?)
}

/// The next free `sort_order` (one past the largest).
pub fn next_sort_order(conn: &Connection) -> AppResult<i64> {
    let max: Option<i64> =
        conn.query_row("SELECT MAX(sort_order) FROM custom_entity_types", [], |r| {
            r.get(0)
        })?;
    Ok(max.map_or(0, |m| m + 1))
}

pub fn insert_custom(
    conn: &Connection,
    def: &EntityTypeDef,
    sort_order: i64,
    now: &str,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO custom_entity_types
           (key, label, plural, icon, color, fields, sort_order, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
        params![
            def.key,
            def.label,
            def.plural,
            def.icon,
            def.color,
            serde_json::to_string(&def.fields)?,
            sort_order,
            now
        ],
    )?;
    Ok(())
}

/// Overwrites label, plural, icon, colour and fields (the key never changes).
pub fn update_custom(conn: &Connection, def: &EntityTypeDef, now: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE custom_entity_types
         SET label = ?2, plural = ?3, icon = ?4, color = ?5, fields = ?6, updated_at = ?7
         WHERE key = ?1",
        params![
            def.key,
            def.label,
            def.plural,
            def.icon,
            def.color,
            serde_json::to_string(&def.fields)?,
            now
        ],
    )?;
    Ok(())
}

pub fn set_sort_order(conn: &Connection, key: &str, sort_order: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE custom_entity_types SET sort_order = ?2 WHERE key = ?1",
        params![key, sort_order],
    )?;
    Ok(())
}

pub fn delete_custom(conn: &Connection, key: &str) -> AppResult<()> {
    conn.execute(
        "DELETE FROM custom_entity_types WHERE key = ?1",
        params![key],
    )?;
    Ok(())
}

/// Extra user fields of a built-in type (empty when none are stored).
pub fn get_template(conn: &Connection, type_key: &str) -> AppResult<Vec<FieldDef>> {
    let text: Option<String> = conn
        .query_row(
            "SELECT fields FROM type_templates WHERE type_key = ?1",
            params![type_key],
            |r| r.get(0),
        )
        .optional()?;
    Ok(text.map(|t| parse_fields(&t)).unwrap_or_default())
}

/// Every stored template as `(type_key, fields)`.
pub fn list_templates(conn: &Connection) -> AppResult<Vec<(String, Vec<FieldDef>)>> {
    let mut stmt = conn.prepare("SELECT type_key, fields FROM type_templates ORDER BY type_key")?;
    let rows = stmt.query_map([], |row| {
        let fields: String = row.get(1)?;
        Ok((row.get::<_, String>(0)?, parse_fields(&fields)))
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Saves the extra fields of a built-in type; an empty list removes the row.
pub fn set_template(conn: &Connection, type_key: &str, fields: &[FieldDef]) -> AppResult<()> {
    if fields.is_empty() {
        conn.execute(
            "DELETE FROM type_templates WHERE type_key = ?1",
            params![type_key],
        )?;
    } else {
        conn.execute(
            "INSERT INTO type_templates (type_key, fields) VALUES (?1, ?2)
             ON CONFLICT(type_key) DO UPDATE SET fields = excluded.fields",
            params![type_key, serde_json::to_string(fields)?],
        )?;
    }
    Ok(())
}

pub fn count_entities(conn: &Connection, type_key: &str) -> AppResult<i64> {
    Ok(conn.query_row(
        "SELECT count(*) FROM entities WHERE type = ?1",
        params![type_key],
        |r| r.get(0),
    )?)
}

/// Ids of all entities of a type.
pub fn entity_ids(conn: &Connection, type_key: &str) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare("SELECT id FROM entities WHERE type = ?1 ORDER BY id")?;
    let rows = stmt.query_map(params![type_key], |r| r.get(0))?;
    Ok(rows.collect::<rusqlite::Result<Vec<String>>>()?)
}

/// Changes the type of every entity of `from` to `to`; returns how many.
pub fn reassign_entities(conn: &Connection, from: &str, to: &str) -> AppResult<usize> {
    Ok(conn.execute(
        "UPDATE entities SET type = ?2 WHERE type = ?1",
        params![from, to],
    )?)
}
