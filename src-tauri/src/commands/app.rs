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

/// End-to-end test hook: the folder named by `ORBIS_E2E_DIR`, if set.
///
/// The Playwright tests cannot click native file dialogs, so the frontend's
/// pick helpers ask this command first and skip the dialog when it answers.
/// Only **debug builds** honour the variable; in a release build this always
/// returns `None`, so the hook cannot be used on an installed app.
#[tauri::command]
#[specta::specta]
pub fn e2e_dir() -> Option<String> {
    e2e_dir_from(std::env::var("ORBIS_E2E_DIR").ok())
}

/// Pure part of [`e2e_dir`] (so it can be tested without touching the environment).
fn e2e_dir_from(value: Option<String>) -> Option<String> {
    if cfg!(debug_assertions) {
        value.filter(|v| !v.trim().is_empty())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_missing_and_blank_values() {
        assert_eq!(e2e_dir_from(None), None);
        assert_eq!(e2e_dir_from(Some("  ".into())), None);
    }

    #[test]
    fn passes_a_value_through_only_in_debug_builds() {
        let got = e2e_dir_from(Some("C:/tmp/x".into()));
        if cfg!(debug_assertions) {
            assert_eq!(got.as_deref(), Some("C:/tmp/x"));
        } else {
            assert_eq!(got, None);
        }
    }
}
