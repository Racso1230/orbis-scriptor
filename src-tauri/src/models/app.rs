use serde::{Deserialize, Serialize};
use specta::Type;

/// Name and version of the running app, as read from `tauri.conf.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
}
