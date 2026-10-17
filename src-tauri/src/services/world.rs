//! Creating and opening world folders.
//!
//! A world is a folder containing `world.json`, `world.db` and `assets/`.
//! These functions know nothing about Tauri, so they are tested with plain
//! temporary folders.

use std::fs;
use std::path::Path;

use chrono::Utc;

use crate::db;
use crate::error::{AppError, AppResult};
use crate::models::world::{APP_ID, FORMAT_VERSION, WorldManifest};
use crate::state::WorldHandle;

pub const MANIFEST_FILE: &str = "world.json";
pub const DB_FILE: &str = "world.db";
pub const BACKUP_FILE: &str = "world.db.bak";
pub const ASSETS_DIR: &str = "assets";
pub const BACKUPS_DIR: &str = "backups";

/// Creates a new world in `path`. The folder may not exist yet (it is
/// created) or must be empty; anything else is refused.
pub fn create_world(path: &Path, name: &str) -> AppResult<WorldHandle> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::Validation("World name is required".into()));
    }
    if path.join(MANIFEST_FILE).exists() {
        return Err(AppError::Validation(
            "This folder is already a world. Use \"Open world\" instead.".into(),
        ));
    }
    if path.exists() {
        if !path.is_dir() {
            return Err(AppError::Validation(
                "The chosen path is not a folder".into(),
            ));
        }
        // `read_dir` yields an iterator; `.next().is_some()` = "has at least one entry".
        if fs::read_dir(path)?.next().is_some() {
            return Err(AppError::Validation(
                "The folder is not empty and is not a world. Choose an empty folder.".into(),
            ));
        }
    } else {
        fs::create_dir_all(path)?;
    }

    let manifest = WorldManifest {
        app: APP_ID.to_owned(),
        name: name.to_owned(),
        format_version: FORMAT_VERSION,
        created_at: Utc::now().to_rfc3339(),
    };
    fs::write(
        path.join(MANIFEST_FILE),
        serde_json::to_string_pretty(&manifest)?,
    )?;
    fs::create_dir_all(path.join(ASSETS_DIR))?;
    let conn = db::open(&path.join(DB_FILE))?;

    Ok(WorldHandle {
        path: path.to_path_buf(),
        manifest,
        conn,
    })
}

/// Opens an existing world, backing up the database before migrations run.
pub fn open_world(path: &Path) -> AppResult<WorldHandle> {
    let manifest_path = path.join(MANIFEST_FILE);
    if !manifest_path.exists() {
        return Err(AppError::NotFound(format!(
            "No world.json found in {}",
            path.display()
        )));
    }

    let manifest: WorldManifest = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
    if manifest.app != APP_ID {
        return Err(AppError::Validation(
            "world.json was not written by Orbis Scriptor".into(),
        ));
    }
    if manifest.format_version > FORMAT_VERSION {
        return Err(AppError::Validation(format!(
            "This world uses format version {} but this app only understands up to {}. Please update the app.",
            manifest.format_version, FORMAT_VERSION
        )));
    }

    // Safety net: keep a copy of the database as it was before this version
    // of the app touched it. No connection is open yet, so the file is stable.
    let db_path = path.join(DB_FILE);
    if db_path.exists() {
        fs::copy(&db_path, path.join(BACKUP_FILE))?;
    }
    fs::create_dir_all(path.join(ASSETS_DIR))?;
    let conn = db::open(&db_path)?;

    Ok(WorldHandle {
        path: path.to_path_buf(),
        manifest,
        conn,
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn create_then_open_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let world_dir = dir.path().join("MyWorld");

        let created = create_world(&world_dir, "  My World ").unwrap();
        assert_eq!(created.manifest.name, "My World");
        assert!(world_dir.join(MANIFEST_FILE).exists());
        assert!(world_dir.join(DB_FILE).exists());
        assert!(world_dir.join(ASSETS_DIR).is_dir());
        assert!(!world_dir.join(BACKUP_FILE).exists());
        drop(created); // closes the connection

        let opened = open_world(&world_dir).unwrap();
        assert_eq!(opened.manifest.name, "My World");
        assert_eq!(opened.manifest.format_version, FORMAT_VERSION);
        assert_eq!(opened.summary().name, "My World");
        // Opening an existing database makes a backup first.
        assert!(world_dir.join(BACKUP_FILE).exists());
    }

    #[test]
    fn open_rejects_folder_without_manifest() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(open_world(dir.path()), Err(AppError::NotFound(_))));
    }

    #[test]
    fn open_rejects_foreign_or_newer_manifest() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(MANIFEST_FILE),
            r#"{"app":"something-else","name":"x","formatVersion":1,"createdAt":""}"#,
        )
        .unwrap();
        assert!(matches!(
            open_world(dir.path()),
            Err(AppError::Validation(_))
        ));

        fs::write(
            dir.path().join(MANIFEST_FILE),
            r#"{"app":"orbis-scriptor","name":"x","formatVersion":999,"createdAt":""}"#,
        )
        .unwrap();
        assert!(matches!(
            open_world(dir.path()),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn create_rejects_non_empty_folder_and_empty_name() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("stray.txt"), "hi").unwrap();
        assert!(matches!(
            create_world(dir.path(), "World"),
            Err(AppError::Validation(_))
        ));

        let empty = tempfile::tempdir().unwrap();
        assert!(matches!(
            create_world(empty.path(), "   "),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn create_rejects_existing_world() {
        let dir = tempfile::tempdir().unwrap();
        drop(create_world(dir.path(), "World").unwrap());
        assert!(matches!(
            create_world(dir.path(), "Again"),
            Err(AppError::Validation(_))
        ));
    }
}
