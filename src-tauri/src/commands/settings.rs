use std::path::PathBuf;

use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};
use crate::models::settings::AppSettings;
use crate::services::settings;

/// `settings.json` in the per-user app config directory (beside recent worlds).
fn settings_file(app: &AppHandle) -> AppResult<PathBuf> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| AppError::Internal(format!("cannot resolve config dir: {e}")))?;
    Ok(dir.join("settings.json"))
}

/// The saved app settings (defaults if none are saved yet).
#[tauri::command]
#[specta::specta]
pub fn get_settings(app: AppHandle) -> AppResult<AppSettings> {
    Ok(settings::load(&settings_file(&app)?))
}

/// Saves the settings (numbers are clamped) and returns what was stored.
#[tauri::command]
#[specta::specta]
pub fn update_settings(app: AppHandle, settings: AppSettings) -> AppResult<AppSettings> {
    settings::save(&settings_file(&app)?, settings)
}
