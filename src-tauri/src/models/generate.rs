//! Event payloads for a running map generation (see `commands::generate`).

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::models::generator::GenerationStats;

/// Which stage a generation job is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum GeneratePhase {
    /// Building the terrain plan in memory (heights, rivers, coast, stamps).
    Generating,
    /// Saving the background image into the world's `assets/` folder.
    Rendering,
    /// Writing the map, its layers and objects to the database.
    Writing,
}

/// Progress of a running generation. Sent as the event `generate-progress`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct GenerateProgress {
    /// The id returned by `generate_map`.
    pub job_id: String,
    pub phase: GeneratePhase,
    /// Steps done and total steps within the current phase.
    pub done: u32,
    pub total: u32,
}

/// Sent once as the event `generate-finished`, after success or failure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct GenerateFinished {
    pub job_id: String,
    pub ok: bool,
    /// The new map entity (open it in a tab). `None` on failure: a failed
    /// generation changes nothing.
    pub map_entity_id: Option<String>,
    pub stats: Option<GenerationStats>,
    /// What went wrong, in plain words, when `ok` is false.
    pub error: Option<String>,
}
