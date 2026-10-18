use std::path::{Path, PathBuf};

use tauri::State;

use crate::error::AppResult;
use crate::models::gallery::{EntityImage, EntityPortrait};
use crate::services::{assets, gallery, world};
use crate::state::AppState;

/// `<world>/assets`, copied out so the lock can be released straight away.
fn assets_dir(state: &State<'_, AppState>) -> AppResult<PathBuf> {
    state.with_world(|w| Ok(w.path.join(world::ASSETS_DIR)))
}

/// Imports an image file (png, jpg, webp or gif) and appends it to the
/// entity's gallery. Like `import_asset`, the file is copied without the
/// database lock and the thumbnail is made on a background thread, so
/// `thumbPath` is usually `null` in the result (call `list_entity_images`
/// again a moment later).
#[tauri::command]
#[specta::specta]
pub fn add_entity_image(
    state: State<'_, AppState>,
    entity_id: String,
    path: String,
) -> AppResult<EntityImage> {
    let dir = assets_dir(&state)?;
    let asset = assets::copy_into_assets(Path::new(&path), &dir)?;
    let added = state.with_world(|w| gallery::add_new_asset(&w.conn, &dir, &entity_id, &asset));
    match added {
        Ok(image) => {
            assets::spawn_thumbnail(dir, asset.id.clone(), asset.mime.clone());
            Ok(image)
        }
        Err(err) => {
            // Do not leave an orphan file behind if the insert failed.
            assets::delete_files(&dir, &asset.id, &asset.mime);
            Err(err)
        }
    }
}

/// Appends an asset that is already in the world (for example a map
/// background) to the entity's gallery.
#[tauri::command]
#[specta::specta]
pub fn add_entity_image_from_asset(
    state: State<'_, AppState>,
    entity_id: String,
    asset_id: String,
) -> AppResult<EntityImage> {
    state.with_world(|w| {
        gallery::add_from_asset(
            &w.conn,
            &w.path.join(world::ASSETS_DIR),
            &entity_id,
            &asset_id,
        )
    })
}

/// The gallery of an entity in display order, each image with its file
/// `path` and `thumbPath` (for `convertFileSrc`).
#[tauri::command]
#[specta::specta]
pub fn list_entity_images(
    state: State<'_, AppState>,
    entity_id: String,
) -> AppResult<Vec<EntityImage>> {
    state.with_world(|w| gallery::list(&w.conn, &w.path.join(world::ASSETS_DIR), &entity_id))
}

/// Changes an image's caption (`null` clears it; at most 500 characters).
#[tauri::command]
#[specta::specta]
pub fn update_entity_image(
    state: State<'_, AppState>,
    id: String,
    caption: Option<String>,
) -> AppResult<EntityImage> {
    state.with_world(|w| {
        gallery::update_caption(
            &w.conn,
            &w.path.join(world::ASSETS_DIR),
            &id,
            caption.as_deref(),
        )
    })
}

/// Reorders the gallery: `ids` must list every image of the entity once.
#[tauri::command]
#[specta::specta]
pub fn reorder_entity_images(
    state: State<'_, AppState>,
    entity_id: String,
    ids: Vec<String>,
) -> AppResult<Vec<EntityImage>> {
    state.with_world(|w| {
        gallery::reorder(&w.conn, &w.path.join(world::ASSETS_DIR), &entity_id, &ids)
    })
}

/// Removes an image from its gallery. The image file is deleted too, but
/// only when no other gallery image or map background uses the same asset.
#[tauri::command]
#[specta::specta]
pub fn remove_entity_image(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.with_world(|w| gallery::remove(&w.conn, &w.path.join(world::ASSETS_DIR), &id))
}

/// Makes an image of the entity its portrait, or clears the portrait with
/// `null`. Returns the gallery.
#[tauri::command]
#[specta::specta]
pub fn set_portrait(
    state: State<'_, AppState>,
    entity_id: String,
    image_id: Option<String>,
) -> AppResult<Vec<EntityImage>> {
    state.with_world(|w| {
        gallery::set_portrait(
            &w.conn,
            &w.path.join(world::ASSETS_DIR),
            &entity_id,
            image_id.as_deref(),
        )
    })
}

/// Every portrait in the world, for avatars in the graph or on map pins.
#[tauri::command]
#[specta::specta]
pub fn list_portraits(state: State<'_, AppState>) -> AppResult<Vec<EntityPortrait>> {
    state.with_world(|w| gallery::list_portraits(&w.conn, &w.path.join(world::ASSETS_DIR)))
}
