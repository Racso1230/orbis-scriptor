//! Business logic. Services know nothing about Tauri; they take plain inputs
//! and return `AppResult`s, which keeps them easy to unit test.

pub mod app;
pub mod assets;
pub mod backups;
pub mod calendar;
pub mod calendar_manager;
pub mod dev_seed;
pub mod documents;
pub mod entities;
pub mod entity_types;
pub mod events;
pub mod export;
pub mod graph;
pub mod links;
pub mod maps;
pub mod markdown;
pub mod markdown_export;
pub mod properties;
pub mod recent_worlds;
pub mod relationships;
pub mod sample_world;
pub mod search;
pub mod settings;
pub mod world;
