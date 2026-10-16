use serde::{Deserialize, Serialize};
use specta::Type;

/// Value of the `app` field in every `world.json` we write or accept.
pub const APP_ID: &str = "orbis-scriptor";

/// Bump when the on-disk layout changes in a way older builds can't read.
pub const FORMAT_VERSION: u32 = 1;

/// Contents of `world.json` at the root of a world folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WorldManifest {
    pub app: String,
    pub name: String,
    pub format_version: u32,
    /// UTC ISO-8601 text, e.g. `2026-10-16T15:04:05Z`.
    pub created_at: String,
}

/// What the frontend needs to know about the currently open world.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WorldSummary {
    pub name: String,
    /// Absolute path of the world folder, as a string for the frontend.
    pub path: String,
}
