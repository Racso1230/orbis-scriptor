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
];

pub const MIGRATIONS: Migrations<'_> = Migrations::from_slice(MIGRATIONS_SLICE);

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

        assert_eq!(v1, 1);
        assert_eq!(v1, v2);
    }
}
