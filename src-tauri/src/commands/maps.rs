use std::path::{Path, PathBuf};

use tauri::State;

use crate::db;
use crate::error::{AppError, AppResult};
use crate::models::map::{
    Asset, MapAppearance, MapData, MapInfo, MapObject, MapObjectPatch, NewMapObject,
};
use crate::services::{assets, dev_seed, maps, world};
use crate::state::AppState;

/// `<world>/assets`, copied out so the lock can be released straight away.
fn assets_dir(state: &State<'_, AppState>) -> AppResult<PathBuf> {
    state.with_world(|w| Ok(w.path.join(world::ASSETS_DIR)))
}

/// Imports an image file into the open world's `assets/` folder.
///
/// The slow part (validating and copying the file) runs WITHOUT the database
/// lock; the lock is only taken for the quick row insert. The thumbnail is
/// made on a background thread, so `thumbPath` is usually `null` in the
/// returned asset (call `get_asset` later to pick it up).
#[tauri::command]
#[specta::specta]
pub fn import_asset(state: State<'_, AppState>, path: String) -> AppResult<Asset> {
    let dir = assets_dir(&state)?;
    let asset = assets::copy_into_assets(Path::new(&path), &dir)?;
    let inserted = state.with_world(|w| maps::register_asset(&w.conn, &asset));
    if let Err(err) = inserted {
        // Do not leave an orphan file behind if the insert failed.
        let _ = std::fs::remove_file(&asset.path);
        return Err(err);
    }
    assets::spawn_thumbnail(dir, asset.id.clone(), asset.mime.clone());
    Ok(asset)
}

/// One asset with its absolute `path` and (if ready) `thumbPath`.
#[tauri::command]
#[specta::specta]
pub fn get_asset(state: State<'_, AppState>, asset_id: String) -> AppResult<Asset> {
    state.with_world(|w| {
        let asset = db::assets::get(&w.conn, &asset_id)?
            .ok_or_else(|| AppError::NotFound(format!("Asset {asset_id} does not exist")))?;
        Ok(assets::with_paths(asset, &w.path.join(world::ASSETS_DIR)))
    })
}

/// A map (looked up by its entity id) with layers and objects.
#[tauri::command]
#[specta::specta]
pub fn get_map(state: State<'_, AppState>, entity_id: String) -> AppResult<MapData> {
    state.with_world(|w| maps::get_map(&w.conn, &w.path.join(world::ASSETS_DIR), &entity_id))
}

/// Sets the map's background image and resizes the map to the image.
#[tauri::command]
#[specta::specta]
pub fn set_map_background(
    state: State<'_, AppState>,
    entity_id: String,
    asset_id: String,
) -> AppResult<MapInfo> {
    state.with_world(|w| {
        maps::set_background(
            &w.conn,
            &w.path.join(world::ASSETS_DIR),
            &entity_id,
            &asset_id,
        )
    })
}

#[tauri::command]
#[specta::specta]
pub fn create_map_object(state: State<'_, AppState>, input: NewMapObject) -> AppResult<MapObject> {
    state.with_world(|w| maps::create_object(&w.conn, input))
}

#[tauri::command]
#[specta::specta]
pub fn update_map_object(
    state: State<'_, AppState>,
    id: String,
    patch: MapObjectPatch,
) -> AppResult<MapObject> {
    state.with_world(|w| maps::update_object(&w.conn, &id, patch))
}

#[tauri::command]
#[specta::specta]
pub fn delete_map_object(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.with_world(|w| maps::delete_object(&w.conn, &id))
}

/// Every pin that points at `entity_id` (for the "appears on maps" panel).
#[tauri::command]
#[specta::specta]
pub fn list_entity_map_appearances(
    state: State<'_, AppState>,
    entity_id: String,
) -> AppResult<Vec<MapAppearance>> {
    state.with_world(|w| maps::list_appearances(&w.conn, &entity_id))
}

/// Development only: adds `count` random pins to a map for performance tests.
#[tauri::command]
#[specta::specta]
pub fn dev_seed_map_pins(
    state: State<'_, AppState>,
    map_entity_id: String,
    count: u32,
) -> AppResult<u32> {
    if !cfg!(debug_assertions) {
        return Err(AppError::Validation(
            "dev_seed_map_pins is only available in debug builds".into(),
        ));
    }
    state.with_world(|w| dev_seed::seed_map_pins(&w.conn, &map_entity_id, count))
}
