//! Thin `#[tauri::command]` functions: validate input, call a service, return
//! an `AppResult`. Every command also carries `#[specta::specta]` so its
//! TypeScript signature is generated.

pub mod app;
pub mod calendars;
pub mod documents;
pub mod entities;
pub mod events;
pub mod export;
pub mod links;
pub mod maps;
pub mod relationships;
pub mod search;
pub mod settings;
pub mod world;
