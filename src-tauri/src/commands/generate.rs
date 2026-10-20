//! Commands for the "Generate map" wizard (Phases 14a and 14b).

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use tauri::{AppHandle, State};
use tauri_specta::Event;
use uuid::Uuid;

use crate::db;
use crate::error::{AppError, AppResult};
use crate::models::generate::{GenerateFinished, GeneratePhase, GenerateProgress};
use crate::models::generator::{GenerationScope, GeneratorParams};
use crate::services::generator::{self, writer};
use crate::services::world::{ASSETS_DIR, DB_FILE};
use crate::state::AppState;

/// True while a generation runs; only one at a time (each one writes a big
/// transaction and uses a lot of memory).
static GENERATE_RUNNING: AtomicBool = AtomicBool::new(false);

/// Clears [`GENERATE_RUNNING`] when dropped, however the job ends (even if
/// the thread panics, because dropping happens during unwinding too).
struct RunningGuard;

impl Drop for RunningGuard {
    fn drop(&mut self) {
        GENERATE_RUNNING.store(false, Ordering::SeqCst);
    }
}

/// Renders a quick low-resolution preview and returns it as base64 PNG text
/// (the wizard shows it as a `data:` URL). `max_side` is the longer side in
/// pixels, 64..=2048.
///
/// `async` + `spawn_blocking`: the work runs on a worker thread so the
/// window stays responsive while the wizard re-renders on every change.
#[tauri::command]
#[specta::specta]
pub async fn preview_generation(params: GeneratorParams, max_side: u32) -> AppResult<String> {
    tauri::async_runtime::spawn_blocking(move || preview_base64(&params, max_side))
        .await
        .map_err(|e| AppError::Internal(format!("the preview stopped unexpectedly: {e}")))?
}

fn preview_base64(params: &GeneratorParams, max_side: u32) -> AppResult<String> {
    let png = generator::render_preview(params, max_side)?;
    Ok(STANDARD.encode(png))
}

/// Checks the request and returns the name the new map will get.
fn checked_name(params: &GeneratorParams, name: Option<&str>) -> AppResult<String> {
    params.validate()?;
    if params.scope == GenerationScope::History {
        return Err(AppError::Validation(
            "History generation arrives in 14c".into(),
        ));
    }
    // `Option::map` + `filter` turn "missing or blank" into `None`.
    let name = name.map(str::trim).filter(|n| !n.is_empty());
    Ok(match name {
        Some(n) => n.to_owned(),
        None => format!("Generated map {}", params.seed),
    })
}

/// Starts generating a terrain map into the open world and returns a job id
/// at once. The work runs on a background thread and reports through two
/// events (see `models::generate`): `generate-progress` while it runs and
/// `generate-finished` once, with the new map's entity id and stats, or the
/// error text. The map is written in one transaction: all or nothing.
#[tauri::command]
#[specta::specta]
pub fn generate_map(
    app: AppHandle,
    state: State<'_, AppState>,
    params: GeneratorParams,
    name: Option<String>,
) -> AppResult<String> {
    let name = checked_name(&params, name.as_deref())?;
    // Copy the folder path out; the lock is released straight away, so the
    // app stays usable while the map is generated.
    let world_dir = state.with_world(|w| Ok(w.path.clone()))?;
    if GENERATE_RUNNING.swap(true, Ordering::SeqCst) {
        return Err(AppError::Validation(
            "A map is already being generated".into(),
        ));
    }
    let job_id = Uuid::now_v7().to_string();
    let job = job_id.clone();
    // `move` hands ownership of the captured values to the new thread.
    thread::spawn(move || {
        let _guard = RunningGuard;
        let mut on_progress = |phase: GeneratePhase, done: u32, total: u32| {
            // A failed emit (window closed) must not stop the job.
            let _ = GenerateProgress {
                job_id: job.clone(),
                phase,
                done,
                total,
            }
            .emit(&app);
        };
        // A closure that returns `AppResult` lets us use `?` for every step.
        let mut run = || -> AppResult<(String, generator::GeneratedMap)> {
            // Reports `generating` and, for the places scope, `placing`.
            let plan = generator::generate_map_with_progress(&params, &mut on_progress)?;
            // Our own connection: the app's connection stays free for the
            // UI. WAL mode lets both use the database at the same time.
            let conn = db::open(&world_dir.join(DB_FILE))?;
            let id = writer::write_map(
                &conn,
                &world_dir.join(ASSETS_DIR),
                &plan,
                &name,
                &mut on_progress,
            )?;
            Ok((id, plan))
        };
        let finished = match run() {
            Ok((id, plan)) => GenerateFinished {
                job_id: job.clone(),
                ok: true,
                map_entity_id: Some(id),
                stats: Some(plan.stats),
                error: None,
            },
            Err(err) => GenerateFinished {
                job_id: job.clone(),
                ok: false,
                map_entity_id: None,
                stats: None,
                error: Some(err.to_string()),
            },
        };
        let _ = finished.emit(&app);
    });
    Ok(job_id)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::models::generator::GenerationStats;
    use std::collections::BTreeMap;

    #[test]
    fn invalid_params_are_validation_errors() {
        let bad = GeneratorParams {
            land_percent: 5,
            ..GeneratorParams::default()
        };
        assert!(matches!(
            checked_name(&bad, None),
            Err(AppError::Validation(_))
        ));
        let later = GeneratorParams {
            scope: GenerationScope::History,
            ..GeneratorParams::default()
        };
        assert!(matches!(
            checked_name(&later, None),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            preview_base64(&GeneratorParams::default(), 10),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn blank_names_get_a_default() {
        let params = GeneratorParams::default();
        assert_eq!(checked_name(&params, None).unwrap(), "Generated map 42");
        assert_eq!(
            checked_name(&params, Some("  ")).unwrap(),
            "Generated map 42"
        );
        assert_eq!(checked_name(&params, Some(" Aria ")).unwrap(), "Aria");
        let places = GeneratorParams {
            scope: GenerationScope::Places,
            ..GeneratorParams::default()
        };
        assert!(checked_name(&places, None).is_ok());
    }

    #[test]
    fn preview_is_base64_png() {
        let params = GeneratorParams {
            width: 400,
            height: 300,
            ..GeneratorParams::default()
        };
        let text = preview_base64(&params, 64).unwrap();
        let bytes = STANDARD.decode(text).unwrap();
        assert!(bytes.starts_with(b"\x89PNG"));
    }

    #[test]
    fn events_serialise_camel_case_and_round_trip() {
        let progress = GenerateProgress {
            job_id: "j".into(),
            phase: GeneratePhase::Writing,
            done: 3,
            total: 9,
        };
        let json = serde_json::to_value(&progress).unwrap();
        assert_eq!(json["jobId"], "j");
        assert_eq!(json["phase"], "writing");
        let back: GenerateProgress = serde_json::from_value(json).unwrap();
        assert_eq!(back, progress);

        let mut biome_counts = BTreeMap::new();
        biome_counts.insert(crate::models::generator::Biome::Ocean, 10);
        let finished = GenerateFinished {
            job_id: "j".into(),
            ok: true,
            map_entity_id: Some("m".into()),
            stats: Some(GenerationStats {
                land_percent_actual: 41.5,
                land_masses: 2,
                rivers: 7,
                stamps: 90,
                biome_counts,
                settlements: 12,
                roads: 9,
            }),
            error: None,
        };
        let json = serde_json::to_value(&finished).unwrap();
        assert_eq!(json["mapEntityId"], "m");
        assert_eq!(json["stats"]["landMasses"], 2);
        assert_eq!(json["stats"]["biomeCounts"]["ocean"], 10);
        assert_eq!(json["stats"]["settlements"], 12);
        assert_eq!(
            serde_json::to_value(GeneratePhase::Placing).unwrap(),
            "placing"
        );
        let back: GenerateFinished = serde_json::from_value(json).unwrap();
        assert_eq!(back, finished);
    }
}
