//! SQL for the `search_index` FTS5 table (created in migration 005, rebuilt
//! with a `tags` column in migration 012).
//! One row per entity: `name`, `summary`, `tags` (tag names joined by
//! spaces) and `body` (lore plain text).

use rusqlite::types::Value;
use rusqlite::{Connection, params, params_from_iter};

use crate::error::AppResult;
use crate::models::search::{HIGHLIGHT_END, HIGHLIGHT_START, SearchResult};

/// Replaces the index row of one entity. FTS5 tables have no UNIQUE
/// constraints, so "upsert" is delete-then-insert.
pub fn replace(
    conn: &Connection,
    entity_id: &str,
    name: &str,
    summary: &str,
    tags: &str,
    body: &str,
) -> AppResult<()> {
    delete(conn, entity_id)?;
    conn.execute(
        "INSERT INTO search_index (entity_id, name, summary, tags, body)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![entity_id, name, summary, tags, body],
    )?;
    Ok(())
}

pub fn delete(conn: &Connection, entity_id: &str) -> AppResult<()> {
    conn.execute(
        "DELETE FROM search_index WHERE entity_id = ?1",
        params![entity_id],
    )?;
    Ok(())
}

/// Empties the index and refills it from `entities` + `documents` + tags.
pub fn rebuild_all(conn: &Connection) -> AppResult<()> {
    conn.execute("DELETE FROM search_index", [])?;
    conn.execute(
        "INSERT INTO search_index (entity_id, name, summary, tags, body)
         SELECT e.id, e.name, e.summary,
                COALESCE((SELECT group_concat(t.name, ' ')
                          FROM entity_tags et JOIN tags t ON t.id = et.tag_id
                          WHERE et.entity_id = e.id), ''),
                COALESCE(d.plain_text, '')
         FROM entities e LEFT JOIN documents d ON d.entity_id = e.id",
        [],
    )?;
    Ok(())
}

/// Runs a search.
///
/// - `match_expr`: an already-sanitised FTS5 `MATCH` expression, or `None`
///   to list entities by tag only (then the snippet is the summary).
/// - `tag_groups`: one group per `tag:` filter; an entity must carry at
///   least one tag id of EVERY group (AND between groups, OR inside one,
///   because a group holds a tag and its nested children).
///
/// Ranking: `bm25(search_index, 0, 10, 4, 3, 1)` gives per-column weights in
/// column order (entity_id, name, summary, tags, body): a name hit counts 10x
/// a lore hit. bm25 returns *lower = better*, so we sort ascending.
/// `snippet(..., -1, ...)` lets SQLite pick the best column to excerpt.
pub fn query(
    conn: &Connection,
    match_expr: Option<&str>,
    tag_groups: &[Vec<String>],
    limit: u32,
) -> AppResult<Vec<SearchResult>> {
    // The SQL text is assembled from fixed pieces; user text only ever
    // travels as bound parameters.
    let mut args: Vec<Value> = Vec::new();
    let (select, from, order);
    if let Some(expr) = match_expr {
        args.push(Value::Text(expr.to_owned()));
        args.push(Value::Text(HIGHLIGHT_START.to_string()));
        args.push(Value::Text(HIGHLIGHT_END.to_string()));
        select = "SELECT search_index.entity_id, e.type, e.name,
                snippet(search_index, -1, ?2, ?3, '…', 12)";
        from = "FROM search_index JOIN entities e ON e.id = search_index.entity_id
         WHERE search_index MATCH ?1";
        order = "ORDER BY bm25(search_index, 0.0, 10.0, 4.0, 3.0, 1.0)";
    } else {
        select = "SELECT e.id, e.type, e.name, substr(e.summary, 1, 160)";
        from = "FROM entities e WHERE 1 = 1";
        order = "ORDER BY e.type, e.name COLLATE NOCASE";
    }
    let mut sql = format!("{select} {from}");
    for group in tag_groups {
        let marks = vec!["?"; group.len()].join(",");
        sql.push_str(&format!(
            " AND EXISTS (SELECT 1 FROM entity_tags et
                          WHERE et.entity_id = e.id AND et.tag_id IN ({marks}))"
        ));
        args.extend(group.iter().cloned().map(Value::Text));
    }
    sql.push_str(&format!(" {order} LIMIT ?"));
    args.push(Value::Integer(i64::from(limit)));

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(args.iter()), |row| {
        Ok(SearchResult {
            entity_id: row.get(0)?,
            entity_type: row.get(1)?,
            name: row.get(2)?,
            snippet: row.get(3)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}
