//! Image galleries and portraits on entities.
//!
//! - An entity has an ordered list of images; each is an `assets` row (the
//!   same table map backgrounds use) plus an `entity_images` row with a
//!   caption and position.
//! - At most one image per entity is the portrait (`is_portrait`). Nothing
//!   is a portrait until the user picks one.
//! - Asset reference rule: removing an image deletes the asset (row and
//!   files) ONLY when nothing else uses it any more. An asset counts as used
//!   by every `entity_images` row and every map background that points at
//!   it, so an image shared by two entities or reused as a map background
//!   survives until the last user is gone (see [`asset_is_referenced`]).
//! - Gallery changes count as an edit of the entity (`updated_at`).

use std::path::Path;

use chrono::{SecondsFormat, Utc};
use rusqlite::Connection;
use uuid::Uuid;

use crate::db;
use crate::error::{AppError, AppResult};
use crate::models::entity::{Entity, EntitySummary};
use crate::models::gallery::{EntityImage, EntityPortrait, Portrait};
use crate::models::map::Asset;
use crate::services::assets;

/// Most images one entity may have.
pub const MAX_IMAGES: u32 = 100;
/// Longest caption, in characters.
pub const MAX_CAPTION_CHARS: usize = 500;

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn ensure_entity(conn: &Connection, entity_id: &str) -> AppResult<()> {
    if db::entities::get(conn, entity_id)?.is_none() {
        return Err(AppError::NotFound(format!(
            "Entity {entity_id} does not exist"
        )));
    }
    Ok(())
}

fn require(conn: &Connection, id: &str) -> AppResult<EntityImage> {
    db::gallery::get(conn, id)?
        .ok_or_else(|| AppError::NotFound(format!("Image {id} does not exist")))
}

fn clean_caption(caption: &str) -> AppResult<String> {
    let text = caption.trim();
    if text.chars().count() > MAX_CAPTION_CHARS {
        return Err(AppError::Validation(format!(
            "A caption can have at most {MAX_CAPTION_CHARS} characters"
        )));
    }
    Ok(text.to_owned())
}

/// Fills in the file paths (they are not stored in the database).
fn with_paths(mut image: EntityImage, assets_dir: &Path, mime: &str) -> EntityImage {
    image.path = assets::asset_file(assets_dir, &image.asset_id, mime)
        .to_string_lossy()
        .into_owned();
    let thumb = assets::thumb_file(assets_dir, &image.asset_id);
    image.thumb_path = thumb.exists().then(|| thumb.to_string_lossy().into_owned());
    image
}

fn mime_of(conn: &Connection, asset_id: &str) -> AppResult<String> {
    Ok(db::assets::get(conn, asset_id)?
        .map(|a| a.mime)
        .unwrap_or_default())
}

/// The gallery of an entity in display order.
pub fn list(conn: &Connection, assets_dir: &Path, entity_id: &str) -> AppResult<Vec<EntityImage>> {
    ensure_entity(conn, entity_id)?;
    db::gallery::list_for_entity(conn, entity_id)?
        .into_iter()
        .map(|image| {
            let mime = mime_of(conn, &image.asset_id)?;
            Ok(with_paths(image, assets_dir, &mime))
        })
        .collect()
}

/// Appends an existing asset to the gallery WITHOUT its own transaction or
/// `updated_at` change (the caller does both; the import adds thousands of
/// images in one transaction). Checks the image limit.
pub fn append_asset(
    conn: &Connection,
    entity_id: &str,
    asset_id: &str,
    caption: &str,
) -> AppResult<EntityImage> {
    if db::assets::get(conn, asset_id)?.is_none() {
        return Err(AppError::NotFound(format!(
            "Asset {asset_id} does not exist"
        )));
    }
    if db::gallery::count_for_entity(conn, entity_id)? >= MAX_IMAGES {
        return Err(AppError::Validation(format!(
            "An entity can have at most {MAX_IMAGES} images"
        )));
    }
    let image = EntityImage {
        id: Uuid::now_v7().to_string(),
        entity_id: entity_id.to_owned(),
        asset_id: asset_id.to_owned(),
        caption: clean_caption(caption)?,
        sort_order: db::gallery::next_sort_order(conn, entity_id)?,
        is_portrait: false,
        created_at: now(),
        // The next four are not stored in `entity_images`.
        file_name: String::new(),
        width: None,
        height: None,
        path: String::new(),
        thumb_path: None,
    };
    db::gallery::insert(conn, &image)?;
    require(conn, &image.id)
}

/// Adds an already registered asset (for example a map background) to the
/// gallery of an entity.
pub fn add_from_asset(
    conn: &Connection,
    assets_dir: &Path,
    entity_id: &str,
    asset_id: &str,
) -> AppResult<EntityImage> {
    ensure_entity(conn, entity_id)?;
    let tx = conn.unchecked_transaction()?;
    let image = append_asset(&tx, entity_id, asset_id, "")?;
    db::entities::touch(&tx, entity_id, &now())?;
    tx.commit()?;
    let mime = mime_of(conn, asset_id)?;
    Ok(with_paths(image, assets_dir, &mime))
}

/// Registers a freshly copied asset (see `assets::copy_into_assets`) and adds
/// it to the gallery in one transaction. If this fails the caller removes
/// the copied file.
pub fn add_new_asset(
    conn: &Connection,
    assets_dir: &Path,
    entity_id: &str,
    asset: &Asset,
) -> AppResult<EntityImage> {
    ensure_entity(conn, entity_id)?;
    let tx = conn.unchecked_transaction()?;
    db::assets::insert(&tx, asset)?;
    let image = append_asset(&tx, entity_id, &asset.id, "")?;
    db::entities::touch(&tx, entity_id, &now())?;
    tx.commit()?;
    Ok(with_paths(image, assets_dir, &asset.mime))
}

/// Changes the caption of an image (`None` clears it).
pub fn update_caption(
    conn: &Connection,
    assets_dir: &Path,
    id: &str,
    caption: Option<&str>,
) -> AppResult<EntityImage> {
    let image = require(conn, id)?;
    let caption = clean_caption(caption.unwrap_or(""))?;
    let tx = conn.unchecked_transaction()?;
    db::gallery::set_caption(&tx, id, &caption)?;
    db::entities::touch(&tx, &image.entity_id, &now())?;
    tx.commit()?;
    let mime = mime_of(conn, &image.asset_id)?;
    Ok(with_paths(require(conn, id)?, assets_dir, &mime))
}

/// Puts the images in the given order. `ids` must list every image of the
/// entity exactly once. Returns the gallery in its new order.
pub fn reorder(
    conn: &Connection,
    assets_dir: &Path,
    entity_id: &str,
    ids: &[String],
) -> AppResult<Vec<EntityImage>> {
    ensure_entity(conn, entity_id)?;
    let current = db::gallery::list_for_entity(conn, entity_id)?;
    let mut expected: Vec<&str> = current.iter().map(|i| i.id.as_str()).collect();
    let mut given: Vec<&str> = ids.iter().map(String::as_str).collect();
    expected.sort_unstable();
    given.sort_unstable();
    if expected != given {
        return Err(AppError::Validation(
            "The new order must list every image of the entity exactly once".into(),
        ));
    }
    let tx = conn.unchecked_transaction()?;
    for (position, id) in ids.iter().enumerate() {
        db::gallery::set_sort_order(&tx, id, position as u32)?;
    }
    db::entities::touch(&tx, entity_id, &now())?;
    tx.commit()?;
    list(conn, assets_dir, entity_id)
}

/// True when an asset is still used: by a gallery image or as a map
/// background.
pub fn asset_is_referenced(conn: &Connection, asset_id: &str) -> AppResult<bool> {
    Ok(db::gallery::asset_use_count(conn, asset_id)? > 0)
}

/// Deletes the asset (row, file and thumbnail) when nothing uses it.
/// Returns whether it was deleted. Call after the referencing rows are gone
/// (and after the transaction that removed them has committed).
pub fn delete_asset_if_unused(
    conn: &Connection,
    assets_dir: &Path,
    asset_id: &str,
) -> AppResult<bool> {
    if asset_is_referenced(conn, asset_id)? {
        return Ok(false);
    }
    let Some(asset) = db::assets::get(conn, asset_id)? else {
        return Ok(false);
    };
    db::gallery::delete_asset_row(conn, asset_id)?;
    assets::delete_files(assets_dir, &asset.id, &asset.mime);
    Ok(true)
}

/// Removes an image from its gallery. Its asset (and files) are deleted
/// only if no other gallery image or map uses it.
pub fn remove(conn: &Connection, assets_dir: &Path, id: &str) -> AppResult<()> {
    let image = require(conn, id)?;
    let tx = conn.unchecked_transaction()?;
    db::gallery::delete(&tx, id)?;
    db::entities::touch(&tx, &image.entity_id, &now())?;
    tx.commit()?;
    delete_asset_if_unused(conn, assets_dir, &image.asset_id)?;
    Ok(())
}

/// Makes `image_id` the entity's portrait, or clears the portrait with
/// `None`. The image must belong to the entity. Returns the gallery.
pub fn set_portrait(
    conn: &Connection,
    assets_dir: &Path,
    entity_id: &str,
    image_id: Option<&str>,
) -> AppResult<Vec<EntityImage>> {
    ensure_entity(conn, entity_id)?;
    if let Some(id) = image_id {
        let image = require(conn, id)?;
        if image.entity_id != entity_id {
            return Err(AppError::Validation(
                "That image belongs to another entity".into(),
            ));
        }
    }
    let tx = conn.unchecked_transaction()?;
    db::gallery::set_portrait(&tx, entity_id, image_id)?;
    db::entities::touch(&tx, entity_id, &now())?;
    tx.commit()?;
    list(conn, assets_dir, entity_id)
}

// ---- portraits for summaries --------------------------------------------

fn portrait_for(assets_dir: &Path, asset_id: &str, mime: &str) -> Portrait {
    let thumb = assets::thumb_file(assets_dir, asset_id);
    Portrait {
        asset_id: asset_id.to_owned(),
        path: assets::asset_file(assets_dir, asset_id, mime)
            .to_string_lossy()
            .into_owned(),
        thumb_path: thumb.exists().then(|| thumb.to_string_lossy().into_owned()),
    }
}

/// Fills in `portrait` for a list of summaries (one batched query).
pub fn attach_portraits(
    conn: &Connection,
    assets_dir: &Path,
    items: &mut [EntitySummary],
) -> AppResult<()> {
    let ids: Vec<String> = items.iter().map(|e| e.id.clone()).collect();
    let rows = db::gallery::portraits_for(conn, &ids)?;
    for item in items.iter_mut() {
        item.portrait = rows
            .get(&item.id)
            .map(|(asset, mime)| portrait_for(assets_dir, asset, mime));
    }
    Ok(())
}

/// Fills in `portrait` of one entity.
pub fn attach_portrait(conn: &Connection, assets_dir: &Path, entity: &mut Entity) -> AppResult<()> {
    let rows = db::gallery::portraits_for(conn, std::slice::from_ref(&entity.id))?;
    entity.portrait = rows
        .get(&entity.id)
        .map(|(asset, mime)| portrait_for(assets_dir, asset, mime));
    Ok(())
}

/// Every portrait in the world, so avatars can be shown anywhere (graph,
/// map pins) by looking up an entity id.
pub fn list_portraits(conn: &Connection, assets_dir: &Path) -> AppResult<Vec<EntityPortrait>> {
    let mut out: Vec<EntityPortrait> = db::gallery::all_portraits(conn)?
        .into_iter()
        .map(|(entity_id, (asset, mime))| EntityPortrait {
            entity_id,
            portrait: portrait_for(assets_dir, &asset, &mime),
        })
        .collect();
    out.sort_by(|a, b| a.entity_id.cmp(&b.entity_id));
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::db::migrations::MIGRATIONS;
    use crate::models::entity::NewEntity;
    use crate::services::{entities, maps};

    struct World {
        conn: Connection,
        dir: tempfile::TempDir,
    }

    impl World {
        fn new() -> Self {
            let mut conn = Connection::open_in_memory().unwrap();
            conn.pragma_update(None, "foreign_keys", "ON").unwrap();
            MIGRATIONS.to_latest(&mut conn).unwrap();
            World {
                conn,
                dir: tempfile::tempdir().unwrap(),
            }
        }

        fn assets(&self) -> std::path::PathBuf {
            self.dir.path().join("assets")
        }

        fn entity(&self, name: &str) -> String {
            entities::create(
                &self.conn,
                NewEntity {
                    entity_type: "character".into(),
                    name: name.into(),
                },
            )
            .unwrap()
            .id
        }

        /// Writes a small PNG, copies it in as a new asset and appends it.
        fn add_png(&self, entity_id: &str, shade: u8) -> EntityImage {
            let src = self.dir.path().join(format!("src-{shade}.png"));
            image::RgbImage::from_pixel(8, 6, image::Rgb([shade, 20, 30]))
                .save(&src)
                .unwrap();
            let asset = assets::copy_into_assets(&src, &self.assets()).unwrap();
            add_new_asset(&self.conn, &self.assets(), entity_id, &asset).unwrap()
        }

        fn file_exists(&self, image: &EntityImage) -> bool {
            Path::new(&image.path).is_file()
        }
    }

    fn ids(images: &[EntityImage]) -> Vec<String> {
        images.iter().map(|i| i.id.clone()).collect()
    }

    #[test]
    fn adds_lists_captions_and_reorders() {
        let w = World::new();
        let e = w.entity("Ada");
        let a = w.add_png(&e, 10);
        let b = w.add_png(&e, 20);
        let c = w.add_png(&e, 30);
        assert_eq!((a.sort_order, b.sort_order, c.sort_order), (0, 1, 2));
        assert_eq!(a.file_name, "src-10.png");
        assert_eq!((a.width, a.height), (Some(8), Some(6)));
        assert!(w.file_exists(&a));

        let listed = list(&w.conn, &w.assets(), &e).unwrap();
        assert_eq!(ids(&listed), vec![a.id.clone(), b.id.clone(), c.id.clone()]);

        let updated = update_caption(&w.conn, &w.assets(), &b.id, Some("  The gate ")).unwrap();
        assert_eq!(updated.caption, "The gate");
        assert_eq!(
            update_caption(&w.conn, &w.assets(), &b.id, None)
                .unwrap()
                .caption,
            ""
        );
        assert!(update_caption(&w.conn, &w.assets(), &b.id, Some(&"x".repeat(501))).is_err());

        let order = vec![c.id.clone(), a.id.clone(), b.id.clone()];
        let reordered = reorder(&w.conn, &w.assets(), &e, &order).unwrap();
        assert_eq!(ids(&reordered), order);
        assert_eq!(
            reordered.iter().map(|i| i.sort_order).collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        // A new image goes to the end.
        let d = w.add_png(&e, 40);
        assert_eq!(d.sort_order, 3);

        // The order must be complete and free of strangers or duplicates.
        let other = w.entity("Bob");
        let foreign = w.add_png(&other, 50);
        for bad in [
            vec![a.id.clone()],
            vec![
                a.id.clone(),
                a.id.clone(),
                b.id.clone(),
                c.id.clone(),
                d.id.clone(),
            ],
            vec![a.id.clone(), b.id.clone(), c.id.clone(), foreign.id.clone()],
        ] {
            assert!(matches!(
                reorder(&w.conn, &w.assets(), &e, &bad),
                Err(AppError::Validation(_))
            ));
        }
        assert!(matches!(
            list(&w.conn, &w.assets(), "missing"),
            Err(AppError::NotFound(_))
        ));
    }

    #[test]
    fn portrait_is_exclusive_optional_and_belongs_to_the_entity() {
        let w = World::new();
        let e = w.entity("Ada");
        let a = w.add_png(&e, 10);
        let b = w.add_png(&e, 20);
        assert!(!a.is_portrait && !b.is_portrait);

        let g = set_portrait(&w.conn, &w.assets(), &e, Some(&a.id)).unwrap();
        assert_eq!(g.iter().filter(|i| i.is_portrait).count(), 1);
        assert!(g[0].is_portrait);
        let g = set_portrait(&w.conn, &w.assets(), &e, Some(&b.id)).unwrap();
        assert_eq!(g.iter().filter(|i| i.is_portrait).count(), 1);
        assert!(g[1].is_portrait);

        let mut summary = entities::list(&w.conn, None).unwrap();
        attach_portraits(&w.conn, &w.assets(), &mut summary).unwrap();
        let p = summary[0].portrait.clone().unwrap();
        assert_eq!(p.asset_id, b.asset_id);
        assert!(Path::new(&p.path).is_file());
        let all = list_portraits(&w.conn, &w.assets()).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].entity_id, e);
        let mut entity = entities::get(&w.conn, &e).unwrap();
        attach_portrait(&w.conn, &w.assets(), &mut entity).unwrap();
        assert!(entity.portrait.is_some());

        // Clearing leaves the images.
        let g = set_portrait(&w.conn, &w.assets(), &e, None).unwrap();
        assert_eq!(g.len(), 2);
        assert!(g.iter().all(|i| !i.is_portrait));
        assert!(list_portraits(&w.conn, &w.assets()).unwrap().is_empty());

        // Another entity's image cannot become this entity's portrait.
        let other = w.entity("Bob");
        let foreign = w.add_png(&other, 50);
        assert!(matches!(
            set_portrait(&w.conn, &w.assets(), &e, Some(&foreign.id)),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            set_portrait(&w.conn, &w.assets(), &e, Some("missing")),
            Err(AppError::NotFound(_))
        ));

        // The database itself refuses a second portrait.
        set_portrait(&w.conn, &w.assets(), &e, Some(&a.id)).unwrap();
        let second = w.conn.execute(
            "UPDATE entity_images SET is_portrait = 1 WHERE entity_id = ?1",
            [&e],
        );
        assert!(second.is_err());
    }

    #[test]
    fn removing_an_image_deletes_the_asset_only_when_unreferenced() {
        let w = World::new();
        let ada = w.entity("Ada");
        let bob = w.entity("Bob");
        let a = w.add_png(&ada, 10);
        // Bob reuses Ada's asset.
        let shared = add_from_asset(&w.conn, &w.assets(), &bob, &a.asset_id).unwrap();
        assert!(asset_is_referenced(&w.conn, &a.asset_id).unwrap());

        remove(&w.conn, &w.assets(), &a.id).unwrap();
        assert!(w.file_exists(&shared), "still used by Bob");
        assert!(db::assets::get(&w.conn, &a.asset_id).unwrap().is_some());

        // A map background counts as a reference too.
        let map = entities::create(
            &w.conn,
            NewEntity {
                entity_type: "map".into(),
                name: "World".into(),
            },
        )
        .unwrap();
        maps::set_background(&w.conn, &w.assets(), &map.id, &a.asset_id).unwrap();
        remove(&w.conn, &w.assets(), &shared.id).unwrap();
        assert!(w.file_exists(&shared), "still used as a map background");
        assert!(asset_is_referenced(&w.conn, &a.asset_id).unwrap());

        // An unreferenced asset goes: row, file and thumbnail.
        let lone = w.add_png(&ada, 99);
        assets::make_thumbnail(&w.assets(), &lone.asset_id, "image/png").unwrap();
        let thumb = assets::thumb_file(&w.assets(), &lone.asset_id);
        assert!(thumb.is_file());
        remove(&w.conn, &w.assets(), &lone.id).unwrap();
        assert!(!w.file_exists(&lone));
        assert!(!thumb.exists());
        assert!(db::assets::get(&w.conn, &lone.asset_id).unwrap().is_none());
        assert!(matches!(
            remove(&w.conn, &w.assets(), &lone.id),
            Err(AppError::NotFound(_))
        ));
    }

    #[test]
    fn deleting_an_entity_cascades_to_its_images_and_cleans_assets() {
        let w = World::new();
        let ada = w.entity("Ada");
        let bob = w.entity("Bob");
        let own = w.add_png(&ada, 10);
        let shared = w.add_png(&ada, 20);
        add_from_asset(&w.conn, &w.assets(), &bob, &shared.asset_id).unwrap();
        set_portrait(&w.conn, &w.assets(), &ada, Some(&own.id)).unwrap();

        entities::delete_with_assets(&w.conn, &w.assets(), &ada).unwrap();
        let rows: i64 = w
            .conn
            .query_row("SELECT count(*) FROM entity_images", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 1, "only Bob's image is left");
        assert!(!w.file_exists(&own), "Ada's own file is gone");
        assert!(w.file_exists(&shared), "the shared file stays");

        // A plain delete cascades the rows as well.
        entities::delete(&w.conn, &bob).unwrap();
        let rows: i64 = w
            .conn
            .query_row("SELECT count(*) FROM entity_images", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 0);
    }

    #[test]
    fn gallery_changes_count_as_edits_and_the_size_is_limited() {
        let w = World::new();
        let e = w.entity("Ada");
        let before = entities::get(&w.conn, &e).unwrap().updated_at;
        std::thread::sleep(std::time::Duration::from_millis(3));
        let a = w.add_png(&e, 10);
        let after = entities::get(&w.conn, &e).unwrap().updated_at;
        assert!(after > before);

        for _ in 1..MAX_IMAGES {
            add_from_asset(&w.conn, &w.assets(), &e, &a.asset_id).unwrap();
        }
        assert!(matches!(
            add_from_asset(&w.conn, &w.assets(), &e, &a.asset_id),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            add_from_asset(&w.conn, &w.assets(), &e, "missing"),
            Err(AppError::NotFound(_))
        ));
    }
}
