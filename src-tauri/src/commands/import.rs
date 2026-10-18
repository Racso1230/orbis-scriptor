use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use tauri::{AppHandle, State};
use tauri_specta::Event;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::import::{ImportFinished, ImportOptions, ImportPreview, ImportProgress};
use crate::services::vault_import;
use crate::state::AppState;

/// True while an import runs; only one import at a time is allowed because
/// each one holds the database write lock while it writes.
static IMPORT_RUNNING: AtomicBool = AtomicBool::new(false);

/// Clears [`IMPORT_RUNNING`] when dropped, however the import ends.
struct RunningGuard;

impl Drop for RunningGuard {
    fn drop(&mut self) {
        IMPORT_RUNNING.store(false, Ordering::SeqCst);
    }
}

/// Reports what importing a vault folder would do, without changing the
/// world: notes per type (and which types would be created), folders that
/// matched no type, images, tags, relationships and the first warnings.
///
/// Reading a big vault takes a moment, so this is an `async` command: the
/// work runs on a worker thread and the window stays responsive.
#[tauri::command]
#[specta::specta]
pub async fn preview_import(
    state: State<'_, AppState>,
    vault_dir: String,
    options: ImportOptions,
) -> AppResult<ImportPreview> {
    let world_dir = state.with_world(|w| Ok(w.path.clone()))?;
    tauri::async_runtime::spawn_blocking(move || {
        vault_import::preview(&world_dir, Path::new(&vault_dir), &options)
    })
    .await
    .map_err(|e| AppError::Internal(format!("the preview stopped unexpectedly: {e}")))?
}

/// Starts importing a Markdown / Obsidian vault into the open world and
/// returns a job id at once. The work runs on a background thread inside ONE
/// database transaction (all or nothing) and reports through two events
/// (see `models::import`): `import-progress` (`ImportProgress`) while it
/// runs and `import-finished` (`ImportFinished`) once, with the counts and
/// warnings or the error text. Refuses to start while another import runs.
#[tauri::command]
#[specta::specta]
pub fn import_vault(
    app: AppHandle,
    state: State<'_, AppState>,
    vault_dir: String,
    options: ImportOptions,
) -> AppResult<String> {
    if !Path::new(&vault_dir).is_dir() {
        return Err(AppError::Validation(
            "The vault folder does not exist".into(),
        ));
    }
    let world_dir = state.with_world(|w| Ok(w.path.clone()))?;
    if IMPORT_RUNNING.swap(true, Ordering::SeqCst) {
        return Err(AppError::Validation("An import is already running".into()));
    }
    let job_id = Uuid::now_v7().to_string();
    let job = job_id.clone();
    // `move` hands ownership of the captured values to the new thread.
    thread::spawn(move || {
        let _guard = RunningGuard;
        let mut on_progress = |done: u32, total: u32, phase: &str| {
            // A failed emit (window closed) must not stop the import.
            let _ = ImportProgress {
                job_id: job.clone(),
                done,
                total,
                phase: phase.to_owned(),
            }
            .emit(&app);
            Ok(())
        };
        let result = vault_import::import(
            &world_dir,
            Path::new(&vault_dir),
            &options,
            &mut on_progress,
        );
        let finished = match result {
            Ok(summary) => ImportFinished {
                job_id: job.clone(),
                ok: true,
                created: summary.created,
                skipped: summary.skipped,
                images: summary.images,
                relationships: summary.relationships,
                types_created: summary.types_created,
                warnings: summary.warnings,
                more_warnings: summary.more_warnings,
                error: None,
            },
            Err(err) => ImportFinished {
                job_id: job.clone(),
                ok: false,
                created: 0,
                skipped: 0,
                images: 0,
                relationships: 0,
                types_created: 0,
                warnings: Vec::new(),
                more_warnings: 0,
                error: Some(err.to_string()),
            },
        };
        let _ = finished.emit(&app);
    });
    Ok(job_id)
}
