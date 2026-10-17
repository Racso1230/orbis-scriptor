//! Database layer: SQL and row mapping live here and nowhere else.

pub mod assets;
pub mod documents;
pub mod entities;
pub mod links;
pub mod maps;
pub mod migrations;
pub mod search;

use std::path::Path;

use rusqlite::Connection;

use crate::error::AppResult;

/// Opens (or creates) the SQLite file at `path`, applies the connection
/// settings every connection must have, and brings the schema up to date.
///
/// `&Path` is a *borrowed* path: the caller keeps ownership and we only read
/// it. The owned counterpart is `PathBuf` (like `&str` vs `String`).
pub fn open(path: &Path) -> AppResult<Connection> {
    let mut conn = Connection::open(path)?;

    // Enforce foreign keys (off by default in SQLite) and use WAL journaling,
    // which is faster and lets readers run while a write is in progress.
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "journal_mode", "WAL")?;

    migrations::MIGRATIONS.to_latest(&mut conn)?;
    Ok(conn)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn open_creates_file_and_applies_settings() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("world.db");

        let conn = open(&path).unwrap();

        assert!(path.exists());
        let fk: i64 = conn
            .pragma_query_value(None, "foreign_keys", |r| r.get(0))
            .unwrap();
        let journal: String = conn
            .pragma_query_value(None, "journal_mode", |r| r.get(0))
            .unwrap();
        let version: u32 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(fk, 1);
        assert_eq!(journal.to_lowercase(), "wal");
        assert_eq!(version, migrations::LATEST_VERSION);

        // The meta table from migration 001 is usable.
        conn.execute("INSERT INTO meta (key, value) VALUES ('a', 'b')", [])
            .unwrap();
    }
}
