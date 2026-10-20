//! Saves a [`GeneratedMap`] plan into a world: the background image goes to
//! `assets/`, everything else (map entity, layers, objects, tag, settings) is
//! written through the existing services in ONE database transaction, so a
//! failure leaves the world exactly as it was.

use std::path::Path;

use rusqlite::Connection;

use super::plan::GeneratedMap;
use crate::db;
use crate::error::AppResult;
use crate::models::generate::GeneratePhase;
use crate::models::map::{Asset, BackgroundKind, MapBackground, MapOp, NewMapObject};
use crate::services::{assets, entities, maps, search, tags};

/// Every generated entity carries this tag, so it is easy to find or delete.
pub const TAG_GENERATED: &str = "generated";

/// `world_settings` keys that let a world say how its map was made.
pub const SETTING_VERSION: &str = "generator.version";
pub const SETTING_PARAMS: &str = "generator.params";
pub const SETTING_LAST_MAP: &str = "generator.last_map_id";

/// Progress callback: `(phase, done, total)`.
///
/// `&mut dyn FnMut(..)` is a borrowed "trait object": any closure that may
/// change its captured variables (`FnMut`), chosen at run time (`dyn`).
pub type GenerateProgressFn<'a> = &'a mut dyn FnMut(GeneratePhase, u32, u32);

/// Writes `plan` as a new map called `name` and returns the map's entity id.
///
/// Steps: save the PNG (phase `rendering`), then one transaction (phase
/// `writing`, one step per object batch). If anything fails the transaction
/// rolls back and the PNG is deleted again.
pub fn write_map(
    conn: &Connection,
    assets_dir: &Path,
    plan: &GeneratedMap,
    name: &str,
    on_progress: GenerateProgressFn,
) -> AppResult<String> {
    on_progress(GeneratePhase::Rendering, 0, 1);
    let file_name = format!("generated-map-{}.png", plan.params.seed);
    let asset = assets::save_png_bytes(assets_dir, &plan.background_png, &file_name)?;
    on_progress(GeneratePhase::Rendering, 1, 1);

    match write_rows(conn, assets_dir, plan, name, &asset, on_progress) {
        Ok(entity_id) => {
            // The thumbnail is only a nicety, made after the commit.
            assets::spawn_thumbnail(assets_dir.to_path_buf(), asset.id, asset.mime);
            Ok(entity_id)
        }
        Err(err) => {
            // The transaction was dropped without commit (= rolled back);
            // the image file is the only thing left to clean up.
            assets::delete_files(assets_dir, &asset.id, &asset.mime);
            Err(err)
        }
    }
}

/// The database half of [`write_map`], all inside one transaction.
fn write_rows(
    conn: &Connection,
    assets_dir: &Path,
    plan: &GeneratedMap,
    name: &str,
    asset: &Asset,
    on_progress: GenerateProgressFn,
) -> AppResult<String> {
    // Steps: map setup, one per batch, settings.
    let batches: usize = plan.layers.iter().map(|l| l.batches().len()).sum();
    let total = (batches + 2) as u32;
    let mut done = 0u32;
    on_progress(GeneratePhase::Writing, done, total);

    // Every function below is a "without its own transaction" variant, so
    // they all run inside this one. Returning early with `?` drops `tx`
    // without commit, and SQLite undoes everything written so far.
    let tx = conn.unchecked_transaction()?;

    let look = MapBackground {
        kind: BackgroundKind::Plain,
        color: None,
    };
    let entity = entities::create_blank_map_in(&tx, name, plan.width, plan.height, &look)?;
    maps::register_asset(&tx, asset)?;
    // The image may be smaller than the map (very large maps); keep the
    // map's own size and let the view stretch the image.
    maps::set_background_sized(
        &tx,
        assets_dir,
        &entity.id,
        &asset.id,
        plan.width,
        plan.height,
    )?;
    let default_layer = maps::get_map(&tx, assets_dir, &entity.id)?
        .layers
        .into_iter()
        .next();
    done += 1;
    on_progress(GeneratePhase::Writing, done, total);

    for (index, layer) in plan.layers.iter().enumerate() {
        // The first plan layer reuses the "Default" layer the map came with.
        let layer_id = match (index, &default_layer) {
            (0, Some(existing)) => maps::rename_layer(&tx, &existing.id, &layer.name)?.id,
            _ => maps::create_layer(&tx, &entity.id, &layer.name)?.id,
        };
        for batch in layer.batches() {
            let ops: Vec<MapOp> = batch
                .iter()
                .map(|object| MapOp::Create {
                    object: NewMapObject {
                        id: None,
                        layer_id: layer_id.clone(),
                        geometry: object.geometry.clone(),
                        style: Some(object.style.clone()),
                        entity_id: None,
                        created_at: None,
                    },
                })
                .collect();
            maps::apply_ops_in(&tx, &entity.id, ops)?;
            done += 1;
            on_progress(GeneratePhase::Writing, done, total);
        }
    }

    let names = tags::normalize_all(&[TAG_GENERATED.to_owned()])?;
    let mut cache = tags::load_cache(&tx)?;
    tags::apply_entity_tags(&tx, &entity.id, &names, &mut cache)?;
    // The search index has a tags column, so re-index after tagging.
    search::reindex_entity(&tx, &entity.id)?;

    db::world_settings::set(&tx, SETTING_VERSION, &plan.generator_version.to_string())?;
    db::world_settings::set(&tx, SETTING_PARAMS, &serde_json::to_string(&plan.params)?)?;
    db::world_settings::set(&tx, SETTING_LAST_MAP, &entity.id)?;

    tx.commit()?;
    on_progress(GeneratePhase::Writing, total, total);
    Ok(entity.id)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::error::AppError;
    use crate::models::generator::GeneratorParams;
    use crate::models::map::{MapGeometry, Point};
    use crate::services::generator::{
        GeneratedObject, LAYER_FEATURES, LAYER_RIVERS, LAYER_TERRAIN, ObjectRole, generate_map,
    };
    use crate::services::world::{self, ASSETS_DIR};
    use crate::state::WorldHandle;
    use serde_json::json;
    use std::fs;
    use std::sync::OnceLock;

    /// One small plan shared by all tests (generating takes a moment).
    fn plan() -> &'static GeneratedMap {
        static PLAN: OnceLock<GeneratedMap> = OnceLock::new();
        PLAN.get_or_init(|| {
            generate_map(&GeneratorParams {
                seed: 1,
                width: 800,
                height: 600,
                ..GeneratorParams::default()
            })
            .unwrap()
        })
    }

    fn new_world(parent: &Path) -> WorldHandle {
        world::create_world(&parent.join("W"), "W").unwrap()
    }

    fn count(conn: &Connection, table: &str) -> i64 {
        conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    }

    fn png_files(dir: &Path) -> usize {
        fs::read_dir(dir)
            .map(|d| {
                d.filter_map(Result::ok)
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    // Thumbnails are made in the background; skip them.
                    .filter(|n| n.ends_with(".png") && !n.ends_with(".thumb.png"))
                    .count()
            })
            .unwrap_or(0)
    }

    fn write(w: &WorldHandle, plan: &GeneratedMap) -> AppResult<String> {
        let mut quiet = |_: GeneratePhase, _: u32, _: u32| {};
        write_map(
            &w.conn,
            &w.path.join(ASSETS_DIR),
            plan,
            "Generated map 1",
            &mut quiet,
        )
    }

    #[test]
    fn writes_a_readable_map() {
        let tmp = tempfile::tempdir().unwrap();
        let w = new_world(tmp.path());
        let assets_dir = w.path.join(ASSETS_DIR);
        let plan = plan();

        let mut phases: Vec<(GeneratePhase, u32, u32)> = Vec::new();
        let mut record = |p: GeneratePhase, d: u32, t: u32| phases.push((p, d, t));
        let id = write_map(&w.conn, &assets_dir, plan, "Generated map 1", &mut record).unwrap();

        let data = maps::get_map(&w.conn, &assets_dir, &id).unwrap();
        let bg = data.map.background.as_ref().expect("background set");
        assert_eq!((bg.width, bg.height), (Some(800), Some(600)));
        assert!(Path::new(&bg.path).is_file());
        assert_eq!((data.map.width, data.map.height), (800, 600));
        assert_eq!(data.map.name, "Generated map 1");

        let names: Vec<&str> = data.layers.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, [LAYER_TERRAIN, LAYER_RIVERS, LAYER_FEATURES]);
        for (layer, planned) in data.layers.iter().zip(&plan.layers) {
            let objects: Vec<&MapGeometry> = data
                .objects
                .iter()
                .filter(|o| o.layer_id == layer.id)
                .map(|o| &o.geometry)
                .collect();
            let expected: Vec<&MapGeometry> = planned.objects.iter().map(|o| &o.geometry).collect();
            // Same objects in the same (drawing) order.
            assert_eq!(objects, expected, "layer {}", layer.name);
        }

        let tags = db::tags::names_for_entity(&w.conn, &id).unwrap();
        assert_eq!(tags, [TAG_GENERATED]);
        let setting = |k| db::world_settings::get(&w.conn, k).unwrap();
        assert_eq!(setting(SETTING_VERSION).as_deref(), Some("1"));
        assert_eq!(setting(SETTING_LAST_MAP).as_deref(), Some(id.as_str()));
        let params: GeneratorParams =
            serde_json::from_str(&setting(SETTING_PARAMS).unwrap()).unwrap();
        assert_eq!(&params, &plan.params);

        // Progress: rendering first, writing ends at total/total.
        assert_eq!(phases[0], (GeneratePhase::Rendering, 0, 1));
        let last = *phases.last().unwrap();
        assert_eq!(last.0, GeneratePhase::Writing);
        assert_eq!(last.1, last.2);
    }

    #[test]
    fn a_failure_mid_write_leaves_nothing_behind() {
        let tmp = tempfile::tempdir().unwrap();
        let w = new_world(tmp.path());
        let assets_dir = w.path.join(ASSETS_DIR);
        let before = (
            count(&w.conn, "entities"),
            count(&w.conn, "maps"),
            count(&w.conn, "assets"),
        );

        // An area with two points is rejected by `apply_ops` validation,
        // after the entity, map, asset and earlier layers were written.
        let mut bad = plan().clone();
        bad.layers[2].objects.push(GeneratedObject {
            geometry: MapGeometry::Area {
                points: vec![Point { x: 1.0, y: 1.0 }, Point { x: 2.0, y: 2.0 }],
            },
            style: json!({}),
            role: ObjectRole::Coast,
        });
        let err = write(&w, &bad).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)), "{err:?}");

        let after = (
            count(&w.conn, "entities"),
            count(&w.conn, "maps"),
            count(&w.conn, "assets"),
        );
        assert_eq!(before, after);
        assert_eq!(count(&w.conn, "map_objects"), 0);
        assert_eq!(png_files(&assets_dir), 0);
        assert!(
            db::world_settings::get(&w.conn, SETTING_LAST_MAP)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn an_unwritable_assets_folder_fails_cleanly() {
        let tmp = tempfile::tempdir().unwrap();
        let w = new_world(tmp.path());
        let before = count(&w.conn, "entities");
        // A plain file where the folder should be: saving the PNG fails.
        let not_a_dir = tmp.path().join("blocked");
        fs::write(&not_a_dir, "x").unwrap();
        let mut quiet = |_: GeneratePhase, _: u32, _: u32| {};
        let result = write_map(&w.conn, &not_a_dir, plan(), "Map", &mut quiet);
        assert!(result.is_err());
        assert_eq!(count(&w.conn, "entities"), before);
        assert_eq!(count(&w.conn, "maps"), 0);
    }

    #[test]
    fn writing_the_same_plan_twice_makes_two_maps() {
        let tmp = tempfile::tempdir().unwrap();
        let w = new_world(tmp.path());
        let a = write(&w, plan()).unwrap();
        let b = write(&w, plan()).unwrap();
        assert_ne!(a, b);
        assert_eq!(count(&w.conn, "maps"), 2);
        assert_eq!(png_files(&w.path.join(ASSETS_DIR)), 2);
        let last = db::world_settings::get(&w.conn, SETTING_LAST_MAP).unwrap();
        assert_eq!(last.as_deref(), Some(b.as_str()));
    }
}
