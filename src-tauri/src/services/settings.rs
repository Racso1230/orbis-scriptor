//! Persisted app settings: one small JSON file in the app config directory,
//! next to the recent-worlds list. Functions take the file path so tests can
//! use a temporary folder.

use std::fs;
use std::path::Path;

use crate::error::AppResult;
use crate::models::settings::{AUTOSAVE_DELAY_RANGE, AppSettings, EDITOR_FONT_SIZE_RANGE};

/// Forces numeric settings into their allowed ranges.
pub fn clamp(mut settings: AppSettings) -> AppSettings {
    // `u32::clamp(min, max)` returns the value limited to that range.
    settings.autosave_delay_ms = settings
        .autosave_delay_ms
        .clamp(AUTOSAVE_DELAY_RANGE.0, AUTOSAVE_DELAY_RANGE.1);
    settings.editor_font_size = settings
        .editor_font_size
        .clamp(EDITOR_FONT_SIZE_RANGE.0, EDITOR_FONT_SIZE_RANGE.1);
    settings
}

/// Loads the settings. A missing or corrupt file means "defaults": settings
/// must never stop the app from starting.
pub fn load(file: &Path) -> AppSettings {
    fs::read_to_string(file)
        .ok()
        .and_then(|text| serde_json::from_str::<AppSettings>(&text).ok())
        .map(clamp)
        .unwrap_or_default()
}

/// Clamps and saves the settings; returns what was actually stored.
pub fn save(file: &Path, settings: AppSettings) -> AppResult<AppSettings> {
    let settings = clamp(settings);
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(file, serde_json::to_string_pretty(&settings)?)?;
    Ok(settings)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::models::settings::Theme;

    #[test]
    fn missing_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let s = load(&dir.path().join("settings.json"));
        assert_eq!(s, AppSettings::default());
        assert_eq!(s.theme, Theme::System);
        assert_eq!(s.autosave_delay_ms, 800);
        assert_eq!(s.editor_font_size, 16);
    }

    #[test]
    fn values_are_clamped() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.json");
        let stored = save(
            &file,
            AppSettings {
                theme: Theme::Dark,
                autosave_delay_ms: 5,
                editor_font_size: 99,
            },
        )
        .unwrap();
        assert_eq!(stored.autosave_delay_ms, 300);
        assert_eq!(stored.editor_font_size, 24);
        assert_eq!(load(&file), stored);
    }

    #[test]
    fn round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("sub").join("settings.json");
        let wanted = AppSettings {
            theme: Theme::Light,
            autosave_delay_ms: 2000,
            editor_font_size: 18,
        };
        save(&file, wanted.clone()).unwrap();
        assert_eq!(load(&file), wanted);
    }

    #[test]
    fn partial_file_fills_in_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.json");
        fs::write(&file, r#"{"theme":"dark"}"#).unwrap();
        let s = load(&file);
        assert_eq!(s.theme, Theme::Dark);
        assert_eq!(s.autosave_delay_ms, 800);
    }

    #[test]
    fn corrupt_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.json");
        fs::write(&file, "not json").unwrap();
        assert_eq!(load(&file), AppSettings::default());
        fs::write(&file, r#"{"theme":"purple"}"#).unwrap();
        assert_eq!(load(&file), AppSettings::default());
    }
}
