use crate::error::AppResult;
use crate::models::app::AppInfo;
use crate::services;

/// Returns the app name and version from `tauri.conf.json`.
///
/// `tauri::AppHandle` is injected by Tauri; the frontend never passes it.
#[tauri::command]
#[specta::specta]
pub fn app_info(app: tauri::AppHandle) -> AppResult<AppInfo> {
    let info = app.package_info();
    Ok(services::app::app_info(
        &info.name,
        &info.version.to_string(),
    ))
}
