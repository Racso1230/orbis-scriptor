//! Options and event payloads for the Markdown world export.

use serde::{Deserialize, Serialize};
use specta::Type;

/// Options for `export_markdown`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MarkdownExportOptions {
    /// Copy the map background images into `<World>/assets/` and embed them.
    pub include_assets: bool,
    /// Export `map` entities (and list pins on them).
    pub include_maps: bool,
    /// Export `event` entities (and list events on the entities involved).
    pub include_events: bool,
    /// Replace an earlier export of the same world in the same place.
    /// Only a folder that an earlier export created is ever replaced.
    pub overwrite: bool,
}

/// Progress of a running export. Sent as the event `export-progress`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct ExportProgress {
    /// The id returned by `export_markdown`.
    pub job_id: String,
    pub done: u32,
    pub total: u32,
    /// `preparing`, `writing`, `assets` or `finishing`.
    pub phase: String,
}

/// Sent once as the event `export-finished`, after success or failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct ExportFinished {
    pub job_id: String,
    pub ok: bool,
    /// The exported folder (`<target>/<World name>`) on success.
    pub path: Option<String>,
    /// Number of files written (notes, index, copied images).
    pub files: u32,
    /// The error message on failure.
    pub error: Option<String>,
}
