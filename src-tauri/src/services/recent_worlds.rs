//! The "recent worlds" list shown on the welcome screen.
//!
//! Stored as a small JSON file in the app's config directory (never inside a
//! world folder). Functions take the file path so tests can point them at a
//! temporary folder.

use std::fs;
use std::path::Path;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::error::AppResult;
use crate::models::world::{RecentWorld, WorldSummary};
use crate::services::world::MANIFEST_FILE;

/// How many entries we keep.
pub const MAX_RECENT: usize = 10;

/// On-disk shape of the file. Private: the frontend sees `RecentWorld`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    name: String,
    path: String,
    last_opened_at: String,
}

fn load(file: &Path) -> Vec<Entry> {
    // A missing or corrupt file just means "no recent worlds"; it must never
    // stop the app from starting, so errors collapse to an empty list.
    fs::read_to_string(file)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save(file: &Path, entries: &[Entry]) -> AppResult<()> {
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(file, serde_json::to_string_pretty(entries)?)?;
    Ok(())
}

/// Moves (or adds) `world` to the top of the list.
pub fn remember(file: &Path, world: &WorldSummary) -> AppResult<()> {
    let mut entries = load(file);
    entries.retain(|e| e.path != world.path);
    entries.insert(
        0,
        Entry {
            name: world.name.clone(),
            path: world.path.clone(),
            last_opened_at: Utc::now().to_rfc3339(),
        },
    );
    entries.truncate(MAX_RECENT);
    save(file, &entries)
}

/// Removes the entry for `path`, if present.
pub fn forget(file: &Path, path: &str) -> AppResult<()> {
    let mut entries = load(file);
    entries.retain(|e| e.path != path);
    save(file, &entries)
}

/// Lists entries, newest first, marking ones whose folder has gone missing.
pub fn list(file: &Path) -> Vec<RecentWorld> {
    load(file)
        .into_iter()
        .map(|e| RecentWorld {
            available: Path::new(&e.path).join(MANIFEST_FILE).exists(),
            name: e.name,
            path: e.path,
            last_opened_at: e.last_opened_at,
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn summary(name: &str, path: &str) -> WorldSummary {
        WorldSummary {
            name: name.into(),
            path: path.into(),
        }
    }

    #[test]
    fn remember_puts_newest_first_and_dedupes_by_path() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("sub").join("recent.json");

        remember(&file, &summary("A", "/a")).unwrap();
        remember(&file, &summary("B", "/b")).unwrap();
        remember(&file, &summary("A renamed", "/a")).unwrap();

        let names: Vec<String> = list(&file).into_iter().map(|w| w.name).collect();
        assert_eq!(names, vec!["A renamed", "B"]);
    }

    #[test]
    fn remember_caps_the_list() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("recent.json");
        for i in 0..(MAX_RECENT + 5) {
            remember(&file, &summary(&format!("W{i}"), &format!("/w{i}"))).unwrap();
        }
        assert_eq!(list(&file).len(), MAX_RECENT);
    }

    #[test]
    fn forget_removes_entry_and_list_flags_missing_folders() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("recent.json");

        // A real world folder (just the manifest is enough for `available`).
        let real = dir.path().join("real");
        fs::create_dir_all(&real).unwrap();
        fs::write(real.join(MANIFEST_FILE), "{}").unwrap();

        remember(&file, &summary("Real", &real.to_string_lossy())).unwrap();
        remember(&file, &summary("Gone", "/does/not/exist")).unwrap();

        let worlds = list(&file);
        assert_eq!(worlds.len(), 2);
        assert!(!worlds[0].available); // "Gone" is newest
        assert!(worlds[1].available);

        forget(&file, "/does/not/exist").unwrap();
        assert_eq!(list(&file).len(), 1);
    }

    #[test]
    fn corrupt_file_is_treated_as_empty() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("recent.json");
        fs::write(&file, "not json").unwrap();
        assert!(list(&file).is_empty());
    }
}
