//! Thin `#[tauri::command]` functions: validate input, call a service, return
//! an `AppResult`. Every command also carries `#[specta::specta]` so its
//! TypeScript signature is generated.

pub mod app;
pub mod documents;
pub mod entities;
pub mod links;
pub mod world;
