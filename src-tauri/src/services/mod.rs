//! Business logic. Services know nothing about Tauri; they take plain inputs
//! and return `AppResult`s, which keeps them easy to unit test.

pub mod app;
pub mod assets;
pub mod backups;
pub mod calendar;
pub mod dev_seed;
pub mod documents;
pub mod entities;
pub mod events;
pub mod links;
pub mod maps;
pub mod recent_worlds;
pub mod search;
pub mod world;
