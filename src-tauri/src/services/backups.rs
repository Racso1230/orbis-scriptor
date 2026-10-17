//! Automatic daily backups of `world.db`.
//!
//! On open we make at most one copy per UTC day in `<world>/backups/` named
//! `world-YYYY-MM-DD.db` and keep only the newest [`KEEP`] files.
//!
//! The copy is made with SQLite's `VACUUM INTO`, run on a SEPARATE read-only
//! connection. That gives a consistent snapshot even while the main
//! connection is open (a plain file copy could catch the WAL file half
//! written). The backup runs on a background thread so opening is not slowed.

use std::fs;
use std::path::{Path, PathBuf};
use std::thread;

use chrono::{NaiveDate, Utc};
use rusqlite::{Connection, OpenFlags};

use crate::error::{AppError, AppResult};
use crate::services::world::{BACKUPS_DIR, DB_FILE};

/// How many daily backups are kept.
pub const KEEP: usize = 5;

fn backup_name(date: NaiveDate) -> String {
    format!("world-{}.db", date.format("%Y-%m-%d"))
}

/// Parses the date out of `world-YYYY-MM-DD.db`; `None` for other files.
fn parse_name(name: &str) -> Option<NaiveDate> {
    let date = name.strip_prefix("world-")?.strip_suffix(".db")?;
    NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()
}

/// Makes today's backup unless it exists, then prunes old ones.
/// Returns the new backup's path, or `None` if one already existed.
pub fn backup_if_needed(world_dir: &Path, today: NaiveDate) -> AppResult<Option<PathBuf>> {
    let dir = world_dir.join(BACKUPS_DIR);
    fs::create_dir_all(&dir)?;
    let target = dir.join(backup_name(today));
    if target.exists() {
        return Ok(None);
    }

    // `VACUUM INTO` refuses to overwrite, and we never want a half-written
    // file to look like a finished backup, so write to a temp name and rename.
    let temp = dir.join(format!("{}.tmp", backup_name(today)));
    let _ = fs::remove_file(&temp);
    let source = Connection::open_with_flags(
        world_dir.join(DB_FILE),
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let temp_str = temp
        .to_str()
        .ok_or_else(|| AppError::Internal("backup path is not valid UTF-8".into()))?;
    source.execute("VACUUM INTO ?1", [temp_str])?;
    drop(source);
    fs::rename(&temp, &target)?;

    prune(&dir, KEEP)?;
    Ok(Some(target))
}

/// Deletes all but the newest `keep` backups (by the date in the file name).
fn prune(dir: &Path, keep: usize) -> AppResult<()> {
    let mut dated: Vec<(NaiveDate, PathBuf)> = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if let Some(date) = parse_name(&entry.file_name().to_string_lossy()) {
            dated.push((date, entry.path()));
        }
    }
    // Newest first, then drop everything after the first `keep`.
    dated.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, path) in dated.into_iter().skip(keep) {
        fs::remove_file(path)?;
    }
    Ok(())
}

/// Runs [`backup_if_needed`] on a background thread. Failures are logged and
/// never reported to the user: a missed backup must not stop a world opening.
pub fn spawn_daily(world_dir: PathBuf) {
    thread::spawn(move || {
        if let Err(e) = backup_if_needed(&world_dir, Utc::now().date_naive()) {
            eprintln!("daily backup failed: {e}");
        }
    });
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::services::world::create_world;

    fn day(d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 3, d).unwrap()
    }

    #[test]
    fn creates_a_valid_copy_and_does_not_duplicate_same_day() {
        let dir = tempfile::tempdir().unwrap();
        // Keep the world open: the backup must work while the DB is in use.
        let world = create_world(dir.path(), "W").unwrap();
        world
            .conn
            .execute(
                "INSERT INTO entities (id, type, name, summary, properties, created_at, updated_at) \
                 VALUES ('e1','note','Backed up','','{}','t','t')",
                [],
            )
            .unwrap();

        let path = backup_if_needed(dir.path(), day(1)).unwrap().unwrap();
        assert!(path.ends_with("world-2026-03-01.db"));
        assert!(backup_if_needed(dir.path(), day(1)).unwrap().is_none());

        let copy = Connection::open(&path).unwrap();
        let name: String = copy
            .query_row("SELECT name FROM entities WHERE id = 'e1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(name, "Backed up");
        let ok: String = copy
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .unwrap();
        assert_eq!(ok, "ok");
    }

    #[test]
    fn keeps_only_the_newest_five() {
        let dir = tempfile::tempdir().unwrap();
        let _world = create_world(dir.path(), "W").unwrap();
        for d in 1..=8 {
            backup_if_needed(dir.path(), day(d)).unwrap();
        }
        let mut names: Vec<String> = fs::read_dir(dir.path().join(BACKUPS_DIR))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(
            names,
            vec![
                "world-2026-03-04.db",
                "world-2026-03-05.db",
                "world-2026-03-06.db",
                "world-2026-03-07.db",
                "world-2026-03-08.db",
            ]
        );
    }

    #[test]
    fn ignores_unrelated_files_when_pruning() {
        let dir = tempfile::tempdir().unwrap();
        let _world = create_world(dir.path(), "W").unwrap();
        let backups = dir.path().join(BACKUPS_DIR);
        fs::create_dir_all(&backups).unwrap();
        fs::write(backups.join("notes.txt"), "keep me").unwrap();
        for d in 1..=7 {
            backup_if_needed(dir.path(), day(d)).unwrap();
        }
        assert!(backups.join("notes.txt").exists());
    }
}
