use serde::{Deserialize, Serialize};
use specta::Type;

/// Colour scheme preference. Serialised as `"light"`, `"dark"` or `"system"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
    #[default]
    System,
}

/// Per-user app preferences (not per-world), stored in the app config dir.
///
/// `#[serde(default)]` on the struct means any field missing from the file
/// falls back to its value in `Default::default()`, so files written by an
/// older version still load after new settings are added.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    pub theme: Theme,
    /// Milliseconds of inactivity before edits are saved.
    pub autosave_delay_ms: u32,
    /// Lore editor font size in pixels.
    pub editor_font_size: u32,
}

pub const AUTOSAVE_DELAY_RANGE: (u32, u32) = (300, 10_000);
pub const EDITOR_FONT_SIZE_RANGE: (u32, u32) = (12, 24);

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            autosave_delay_ms: 800,
            editor_font_size: 16,
        }
    }
}
