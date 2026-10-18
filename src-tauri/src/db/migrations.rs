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
    // 004: links between entities (mentions now, relationships in Phase 9).
    M::up(
        "CREATE TABLE links (
            id          TEXT PRIMARY KEY,
            source_id   TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
            target_id   TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
            kind        TEXT NOT NULL,   -- 'mention' (from documents) or a relationship kind
            label       TEXT,
            created_at  TEXT NOT NULL,
            UNIQUE (source_id, target_id, kind)
        );
        CREATE INDEX idx_links_target ON links(target_id);",
    ),
    // 005: full-text search. One FTS5 row per entity (name, summary, lore).
    // `entity_id` is UNINDEXED: stored but not searchable. `remove_diacritics 2`
    // makes "Elan" match "Élan"; `prefix` builds extra indexes so "dra*" is fast.
    // The index is kept in sync by services::search (see that module), so the
    // backfill below only covers rows that already exist.
    M::up(
        "CREATE VIRTUAL TABLE search_index USING fts5(
            entity_id UNINDEXED,
            name,
            summary,
            body,
            tokenize = 'unicode61 remove_diacritics 2',
            prefix = '2 3'
        );
        INSERT INTO search_index (entity_id, name, summary, body)
            SELECT e.id, e.name, e.summary, COALESCE(d.plain_text, '')
            FROM entities e LEFT JOIN documents d ON d.entity_id = e.id;",
    ),
    // 006: maps. A map is also an entity (type 'map'); `maps` holds its extra
    // data. Pins (`map_objects`) may link to an entity; deleting that entity
    // sets the link to NULL (the pin stays), while deleting the map entity
    // cascades to its maps, layers and objects.
    M::up(
        "CREATE TABLE assets (
            id          TEXT PRIMARY KEY,   -- UUID v7; the file is assets/<id>.<ext>
            file_name   TEXT NOT NULL,      -- original file name, for display
            mime        TEXT NOT NULL,
            width       INTEGER,
            height      INTEGER,
            created_at  TEXT NOT NULL
        );
        CREATE TABLE maps (
            id                  TEXT PRIMARY KEY,
            entity_id           TEXT NOT NULL UNIQUE REFERENCES entities(id) ON DELETE CASCADE,
            background_asset_id TEXT REFERENCES assets(id),
            width               INTEGER NOT NULL,
            height              INTEGER NOT NULL
        );
        CREATE TABLE map_layers (
            id          TEXT PRIMARY KEY,
            map_id      TEXT NOT NULL REFERENCES maps(id) ON DELETE CASCADE,
            name        TEXT NOT NULL,
            sort_order  INTEGER NOT NULL,
            visible     INTEGER NOT NULL DEFAULT 1
        );
        CREATE INDEX idx_map_layers_map ON map_layers(map_id);
        CREATE TABLE map_objects (
            id          TEXT PRIMARY KEY,
            layer_id    TEXT NOT NULL REFERENCES map_layers(id) ON DELETE CASCADE,
            kind        TEXT NOT NULL,       -- 'pin' now; 'path' | 'area' | 'label' in Phase 10
            geometry    TEXT NOT NULL,       -- JSON, in map pixel coordinates
            style       TEXT NOT NULL DEFAULT '{}',
            entity_id   TEXT REFERENCES entities(id) ON DELETE SET NULL,
            created_at  TEXT NOT NULL,
            updated_at  TEXT NOT NULL
        );
        CREATE INDEX idx_map_objects_layer ON map_objects(layer_id);
        CREATE INDEX idx_map_objects_entity ON map_objects(entity_id);",
    ),
    // 007: timeline. Calendars describe how day numbers become dates (JSON
    // definition, see models::calendar). An event is also an entity (type
    // 'event'); `events` holds its dates. Day numbers are days from the
    // world's epoch and may be negative; end_day NULL = single-day event.
    // This migration also inserts ONE default calendar (12 x 30 days) so
    // existing and new worlds both have it. Its id is a fixed UUID v7-shaped
    // string because SQL cannot generate one.
    // Range queries use start_day (indexed); end_day is checked per row.
    M::up(
        r#"CREATE TABLE calendars (
            id          TEXT PRIMARY KEY,
            name        TEXT NOT NULL,
            definition  TEXT NOT NULL,
            is_default  INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE events (
            entity_id   TEXT PRIMARY KEY REFERENCES entities(id) ON DELETE CASCADE,
            calendar_id TEXT NOT NULL REFERENCES calendars(id),
            start_day   INTEGER NOT NULL,
            end_day     INTEGER,
            importance  INTEGER NOT NULL DEFAULT 1
        );
        CREATE INDEX idx_events_start ON events(start_day);
        INSERT INTO calendars (id, name, definition, is_default) VALUES (
            '00000000-0000-7000-8000-000000000001',
            'Standard calendar',
            '{"months":[{"name":"Month 1","days":30},{"name":"Month 2","days":30},{"name":"Month 3","days":30},{"name":"Month 4","days":30},{"name":"Month 5","days":30},{"name":"Month 6","days":30},{"name":"Month 7","days":30},{"name":"Month 8","days":30},{"name":"Month 9","days":30},{"name":"Month 10","days":30},{"name":"Month 11","days":30},{"name":"Month 12","days":30}],"weekdays":["Moonday","Fireday","Waterday","Earthday","Windday","Starday","Sunday"],"eras":[{"name":"Age of Founding","abbreviation":"AF","startDay":0}]}',
            1
        );"#,
    ),
    // 008: relationships (Phase 9) reuse `links`; this index speeds up the
    // graph queries that filter by kind. Lookups by source are already served
    // by the index behind UNIQUE (source_id, target_id, kind).
    M::up("CREATE INDEX idx_links_kind ON links(kind);"),
    // 009: map editor v2 (Phase 10). Blank maps store their look as JSON in
    // `background_style` (NULL = image map); layers can be locked.
    M::up(
        "ALTER TABLE maps ADD COLUMN background_style TEXT;
        ALTER TABLE map_layers ADD COLUMN locked INTEGER NOT NULL DEFAULT 0;",
    ),
    // 010: custom calendars (Phase 11). Calendar names are unique per world
    // (ignoring case). `world_settings` is a small key/value table; it
    // stores which calendar the world is *displayed* in
    // (key 'display_calendar_id').
    M::up(
        "CREATE UNIQUE INDEX idx_calendars_name ON calendars(name COLLATE NOCASE);
        CREATE TABLE world_settings (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );",
    ),
    // 011: user-defined entity types and field templates (Phase 12).
    // `custom_entity_types.fields` is a JSON array of field definitions (all
    // the type's fields). `type_templates` holds EXTRA user fields for a
    // built-in type (the built-in fields stay in code). `entities.type` has
    // no foreign key, so a custom type's key is simply stored there.
    M::up(
        "CREATE TABLE custom_entity_types (
            key         TEXT PRIMARY KEY,           -- 'custom-<slug>', immutable
            label       TEXT NOT NULL,
            plural      TEXT NOT NULL,
            icon        TEXT NOT NULL,
            color       TEXT NOT NULL,
            fields      TEXT NOT NULL DEFAULT '[]', -- JSON array of FieldDef
            sort_order  INTEGER NOT NULL,
            created_at  TEXT NOT NULL,
            updated_at  TEXT NOT NULL
        );
        CREATE TABLE type_templates (
            type_key    TEXT PRIMARY KEY,
            fields      TEXT NOT NULL DEFAULT '[]'  -- JSON array of FieldDef
        );",
    ),
    // 012: tags, favourites, recently edited and image galleries (Phase 13).
    // - `tags` / `entity_tags`: case-insensitive unique tag names; deleting an
    //   entity or a tag removes its `entity_tags` rows.
    // - `entities.favorite` and an index on `updated_at` (the "recent" list).
    // - `entity_images`: gallery images of an entity (an image is an `assets`
    //   row). At most ONE image per entity has `is_portrait = 1` (partial
    //   unique index).
    // - FTS5 tables cannot gain a column with ALTER TABLE, so the search
    //   index is dropped, created again with a `tags` column and refilled
    //   from `entities` + `documents` (no tags exist yet).
    M::up(
        "CREATE TABLE tags (
            id          TEXT PRIMARY KEY,
            name        TEXT NOT NULL UNIQUE COLLATE NOCASE,
            color       TEXT,
            created_at  TEXT NOT NULL
        );
        CREATE TABLE entity_tags (
            entity_id   TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
            tag_id      TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
            PRIMARY KEY (entity_id, tag_id)
        );
        CREATE INDEX idx_entity_tags_tag ON entity_tags(tag_id);
        ALTER TABLE entities ADD COLUMN favorite INTEGER NOT NULL DEFAULT 0;
        CREATE INDEX idx_entities_updated ON entities(updated_at);
        CREATE TABLE entity_images (
            id          TEXT PRIMARY KEY,
            entity_id   TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
            asset_id    TEXT NOT NULL REFERENCES assets(id),
            caption     TEXT NOT NULL DEFAULT '',
            sort_order  INTEGER NOT NULL,
            is_portrait INTEGER NOT NULL DEFAULT 0,
            created_at  TEXT NOT NULL
        );
        CREATE INDEX idx_entity_images_entity ON entity_images(entity_id, sort_order);
        CREATE INDEX idx_entity_images_asset ON entity_images(asset_id);
        CREATE UNIQUE INDEX idx_entity_images_portrait
            ON entity_images(entity_id) WHERE is_portrait = 1;
        DROP TABLE search_index;
        CREATE VIRTUAL TABLE search_index USING fts5(
            entity_id UNINDEXED,
            name,
            summary,
            tags,
            body,
            tokenize = 'unicode61 remove_diacritics 2',
            prefix = '2 3'
        );
        INSERT INTO search_index (entity_id, name, summary, tags, body)
            SELECT e.id, e.name, e.summary, '', COALESCE(d.plain_text, '')
            FROM entities e LEFT JOIN documents d ON d.entity_id = e.id;",
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

    #[test]
    fn upgrading_an_existing_world_adds_the_default_calendar() {
        let mut conn = Connection::open_in_memory().unwrap();
        // Simulate a world created before Phase 7: stop at migration 006.
        MIGRATIONS.to_version(&mut conn, 6).unwrap();
        let has_table: bool = conn
            .query_row(
                "SELECT count(*) > 0 FROM sqlite_master WHERE name = 'calendars'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(!has_table);

        MIGRATIONS.to_latest(&mut conn).unwrap();
        let calendar = crate::db::calendars::get_default(&conn).unwrap().unwrap();
        assert!(calendar.is_default);
        assert_eq!(calendar.definition.months.len(), 12);
        assert_eq!(
            calendar.definition.eras[0].abbreviation.as_deref(),
            Some("AF")
        );
        assert!(crate::services::calendar::validate(&calendar.definition).is_ok());
        assert_eq!(crate::db::calendars::list(&conn).unwrap().len(), 1);
    }

    #[test]
    fn migration_011_adds_type_tables_and_keeps_entities() {
        let mut conn = Connection::open_in_memory().unwrap();
        MIGRATIONS.to_version(&mut conn, 10).unwrap();
        conn.execute_batch(
            "INSERT INTO entities (id, type, name, summary, properties, created_at, updated_at)
               VALUES ('e1', 'character', 'Ada', '', '{}', 't', 't');",
        )
        .unwrap();
        MIGRATIONS.to_latest(&mut conn).unwrap();
        let tables: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master
                 WHERE name IN ('custom_entity_types', 'type_templates')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let entities: i64 = conn
            .query_row("SELECT count(*) FROM entities", [], |r| r.get(0))
            .unwrap();
        assert_eq!((tables, entities), (2, 1));
    }

    #[test]
    fn migration_008_adds_the_kind_index() {
        let mut conn = Connection::open_in_memory().unwrap();
        MIGRATIONS.to_version(&mut conn, 7).unwrap();
        MIGRATIONS.to_latest(&mut conn).unwrap();
        let has_index: bool = conn
            .query_row(
                "SELECT count(*) > 0 FROM sqlite_master WHERE name = 'idx_links_kind'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(has_index);
    }

    #[test]
    fn migration_009_keeps_existing_maps_and_layers() {
        let mut conn = Connection::open_in_memory().unwrap();
        MIGRATIONS.to_version(&mut conn, 8).unwrap();
        conn.execute_batch(
            "INSERT INTO entities (id, type, name, summary, properties, created_at, updated_at)
               VALUES ('e1', 'map', 'Old', '', '{}', 't', 't');
             INSERT INTO maps (id, entity_id, width, height) VALUES ('m1', 'e1', 10, 10);
             INSERT INTO map_layers (id, map_id, name, sort_order) VALUES ('l1', 'm1', 'Default', 0);",
        )
        .unwrap();
        MIGRATIONS.to_latest(&mut conn).unwrap();
        let style: Option<String> = conn
            .query_row("SELECT background_style FROM maps", [], |r| r.get(0))
            .unwrap();
        let locked: i64 = conn
            .query_row("SELECT locked FROM map_layers", [], |r| r.get(0))
            .unwrap();
        assert_eq!((style, locked), (None, 0));
    }
}
