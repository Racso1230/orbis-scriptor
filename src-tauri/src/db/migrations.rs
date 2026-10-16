//! Schema migrations, applied in order. rusqlite_migration stores the current
//! version in SQLite's `user_version` pragma, so there is no migrations table.
//!
//! Rules: never edit a migration that has shipped; add a new `M::up(...)` at
//! the end instead.

use rusqlite_migration::{M, Migrations};

/// `const` means this is evaluated at compile time and lives for the whole
/// program (`'_` here is a lifetime that the compiler fills in as `'static`).
const MIGRATIONS_SLICE: &[M<'_>] = &[
    // 001: key/value table for world-level settings and bookkeeping.
    M::up(
        "CREATE TABLE meta (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );",
    ),
    // 002: entities — characters, locations, factions, items, notes.
    M::up(
        "CREATE TABLE entities (
            id          TEXT PRIMARY KEY,           -- UUID v7
            type        TEXT NOT NULL,              -- see models::entity_type
            name        TEXT NOT NULL,
            summary     TEXT NOT NULL DEFAULT '',
            properties  TEXT NOT NULL DEFAULT '{}', -- JSON, type-specific fields
            created_at  TEXT NOT NULL,
            updated_at  TEXT NOT NULL
        );
        CREATE INDEX idx_entities_type_name ON entities(type, name);",
    ),
    // 003: one rich-text lore document per entity.
    M::up(
        "CREATE TABLE documents (
            entity_id   TEXT PRIMARY KEY REFERENCES entities(id) ON DELETE CASCADE,
            content     TEXT NOT NULL,   -- TipTap JSON
            plain_text  TEXT NOT NULL,   -- extracted for search
            updated_at  TEXT NOT NULL
        );",
    ),
];

pub const MIGRATIONS: Migrations<'_> = Migrations::from_slice(MIGRATIONS_SLICE);

/// The schema version a fully migrated database reports in `user_version`.
pub const LATEST_VERSION: u32 = MIGRATIONS_SLICE.len() as u32;

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn migrations_are_valid() {
        assert!(MIGRATIONS.validate().is_ok());
    }

    #[test]
    fn applying_twice_is_a_no_op() {
        let mut conn = Connection::open_in_memory().unwrap();
        MIGRATIONS.to_latest(&mut conn).unwrap();
        let v1: u32 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();

        MIGRATIONS.to_latest(&mut conn).unwrap();
        let v2: u32 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();

        assert_eq!(v1, LATEST_VERSION);
        assert_eq!(v1, v2);
    }
}
