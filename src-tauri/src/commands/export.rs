use std::path::Path;
use std::thread;

use tauri::{AppHandle, State};
use tauri_specta::Event;
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::export::{ExportFinished, ExportProgress, MarkdownExportOptions};
use crate::services::{export, markdown_export};
use crate::state::AppState;

/// Writes an exported PNG (base64 text) to a path chosen with the native
/// save dialog. Does not need an open world.
#[tauri::command]
#[specta::specta]
pub fn save_export(path: String, png_base64: String) -> AppResult<()> {
    export::save_png(&path, &png_base64)
}

/// Starts a Markdown export of the open world into `<target_dir>/<World name>/`
/// and returns a job id immediately. The work runs on a background thread and
/// reports through two events (see `models::export`):
/// `export-progress` (`ExportProgress`) while it runs and `export-finished`
/// (`ExportFinished`) once, with `ok`, the folder `path` and the file count,
/// or the error text.
#[tauri::command]
#[specta::specta]
pub fn export_markdown(
    app: AppHandle,
    state: State<'_, AppState>,
    target_dir: String,
    options: MarkdownExportOptions,
) -> AppResult<String> {
    // Take what the thread needs (plain owned values) and release the lock;
    // the thread opens its own read-only connection to world.db.
    let (world_dir, world_name) =
        state.with_world(|w| Ok((w.path.clone(), w.manifest.name.clone())))?;
    let job_id = Uuid::now_v7().to_string();
    let job = job_id.clone();
    // `move` hands ownership of the captured values to the new thread.
    thread::spawn(move || {
        let mut on_progress = |done: u32, total: u32, phase: &str| {
            // A failed emit (window closed) must not stop the export.
            let _ = ExportProgress {
                job_id: job.clone(),
                done,
                total,
                phase: phase.to_owned(),
            }
            .emit(&app);
        };
        let result = markdown_export::export_markdown(
            &world_dir,
            &world_name,
            Path::new(&target_dir),
            &options,
            &mut on_progress,
        );
        let finished = match result {
            Ok(summary) => ExportFinished {
                job_id: job.clone(),
                ok: true,
                path: Some(summary.path.to_string_lossy().into_owned()),
                files: summary.files,
                error: None,
            },
            Err(err) => ExportFinished {
                job_id: job.clone(),
                ok: false,
                path: None,
                files: 0,
                error: Some(err.to_string()),
            },
        };
        let _ = finished.emit(&app);
    });
    Ok(job_id)
}
