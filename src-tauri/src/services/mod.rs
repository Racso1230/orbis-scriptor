//! Business logic. Services know nothing about Tauri; they take plain inputs
//! and return `AppResult`s, which keeps them easy to unit test.

pub mod app;
pub mod entities;
pub mod recent_worlds;
pub mod world;
