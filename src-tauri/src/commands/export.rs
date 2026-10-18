use crate::error::AppResult;
use crate::services::export;

/// Writes an exported PNG (base64 text) to a path chosen with the native
/// save dialog. Does not need an open world.
#[tauri::command]
#[specta::specta]
pub fn save_export(path: String, png_base64: String) -> AppResult<()> {
    export::save_png(&path, &png_base64)
}
