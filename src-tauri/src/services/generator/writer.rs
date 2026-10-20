//! Saves a [`GeneratedMap`] plan into a world: the background image goes to
//! `assets/`, everything else (map entity, location entities, kingdoms,
//! people, events, relationships, lore, layers, objects, tags, settings) is
//! written through the existing services in ONE database transaction, so a
//! failure leaves the world exactly as it was.

use std::path::Path;

use rusqlite::Connection;

use serde_json::json;

use super::plan::{GeneratedHistory, GeneratedMap, GeneratedPlaces, HistDate, ObjectRole, Ref};
use crate::db;
use crate::error::{AppError, AppResult};
use crate::models::calendar::{Calendar, DateStyle};
use crate::models::entity::{EntityPatch, NewEntity};
use crate::models::generate::GeneratePhase;
use crate::models::map::{Asset, BackgroundKind, MapBackground, MapOp, NewMapObject};
use crate::services::{
    assets, calendar, calendar_manager, documents, entities, events, maps, relationships, search,
    tags,
};

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

/// Location entities are created this many at a time per progress step.
const ENTITIES_PER_STEP: usize = 25;

/// Progress steps of the history part: factions, characters, events,
/// relationships, documents.
const HISTORY_STEPS: usize = 5;

/// The message shown when generating into a map that is not empty.
pub const TARGET_NOT_EMPTY: &str = "This map already has content. Generate into a new map instead.";

/// Checks that `entity_id` is a `map` entity whose map has no background
/// image and no objects, so generated terrain can be written into it.
/// The writer calls it inside its transaction; the command also calls it up
/// front so a bad target fails at once instead of after generating.
pub fn check_target(conn: &Connection, entity_id: &str) -> AppResult<()> {
    let entity = db::entities::get(conn, entity_id)?
        .ok_or_else(|| AppError::NotFound(format!("Entity {entity_id} does not exist")))?;
    if entity.entity_type != "map" {
        return Err(AppError::Validation(
            "Terrain can only be generated into a map".into(),
        ));
    }
    let info = db::maps::get_map_by_entity(conn, entity_id)?
        .ok_or_else(|| AppError::NotFound(format!("No map for entity {entity_id}")))?;
    if info.background.is_some() || db::maps::count_objects(conn, &info.id)? > 0 {
        return Err(AppError::Validation(TARGET_NOT_EMPTY.into()));
    }
    Ok(())
}

/// Writes `plan` as a new map called `name` and returns the map's entity id.
/// With `target` (an existing, empty map entity's id) the plan is written
/// into that map instead; `name` is then ignored and the map keeps its name.
///
/// Steps: save the PNG (phase `rendering`), then one transaction (phase
/// `writing`, one step per object batch). If anything fails the transaction
/// rolls back and the PNG is deleted again.
pub fn write_map(
    conn: &Connection,
    assets_dir: &Path,
    plan: &GeneratedMap,
    name: &str,
    target: Option<&str>,
    on_progress: GenerateProgressFn,
) -> AppResult<String> {
    on_progress(GeneratePhase::Rendering, 0, 1);
    let file_name = format!("generated-map-{}.png", plan.params.seed);
    let asset = assets::save_png_bytes(assets_dir, &plan.background_png, &file_name)?;
    on_progress(GeneratePhase::Rendering, 1, 1);

    match write_rows(conn, assets_dir, plan, name, target, &asset, on_progress) {
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
    target: Option<&str>,
    asset: &Asset,
    on_progress: GenerateProgressFn,
) -> AppResult<String> {
    // Steps: map setup, location entities, one per batch, settings.
    let batches: usize = plan.layers.iter().map(|l| l.batches().len()).sum();
    let entity_steps = plan
        .places
        .as_ref()
        .map_or(0, |p| p.settlements.len().div_ceil(ENTITIES_PER_STEP));
    let history_steps = if plan.history.is_some() {
        HISTORY_STEPS
    } else {
        0
    };
    let total = (batches + entity_steps + history_steps + 2) as u32;
    let mut done = 0u32;
    on_progress(GeneratePhase::Writing, done, total);

    // Every function below is a "without its own transaction" variant, so
    // they all run inside this one. Returning early with `?` drops `tx`
    // without commit, and SQLite undoes everything written so far.
    let tx = conn.unchecked_transaction()?;

    let entity_id = match target {
        // Checked again here, inside the transaction, in case the map got
        // content while the plan was being generated.
        Some(id) => {
            check_target(&tx, id)?;
            id.to_owned()
        }
        None => {
            let look = MapBackground {
                kind: BackgroundKind::Plain,
                color: None,
            };
            entities::create_blank_map_in(&tx, name, plan.width, plan.height, &look)?.id
        }
    };
    maps::register_asset(&tx, asset)?;
    // The image may be smaller than the map (very large maps); keep the
    // map's own size and let the view stretch the image.
    maps::set_background_sized(
        &tx,
        assets_dir,
        &entity_id,
        &asset.id,
        plan.width,
        plan.height,
    )?;
    let default_layer = maps::get_map(&tx, assets_dir, &entity_id)?
        .layers
        .into_iter()
        .next();
    done += 1;
    on_progress(GeneratePhase::Writing, done, total);

    let names = tags::normalize_all(&[TAG_GENERATED.to_owned()])?;
    let mut cache = tags::load_cache(&tx)?;

    // One location entity per settlement; `location_ids[i]` belongs to
    // settlement `i`, so pins can be linked to them below.
    let mut location_ids: Vec<String> = Vec::new();
    if let Some(places) = &plan.places {
        let mut step = |done: &mut u32| {
            *done += 1;
            on_progress(GeneratePhase::Writing, *done, total);
        };
        location_ids = write_locations(&tx, places, &names, &mut cache, |n| {
            if n % ENTITIES_PER_STEP == 0 {
                step(&mut done);
            }
        })?;
        if places.settlements.len() % ENTITIES_PER_STEP != 0 {
            step(&mut done);
        }
    }

    // Kingdoms, people, events, relationships and lore. The kingdom areas
    // below link to the faction ids created here.
    let mut faction_ids: Vec<String> = Vec::new();
    if let (Some(history), Some(places)) = (&plan.history, &plan.places) {
        let mut step = || {
            done += 1;
            on_progress(GeneratePhase::Writing, done, total);
        };
        let ids = write_history(
            &tx,
            history,
            places,
            &location_ids,
            &names,
            &mut cache,
            &mut step,
        )?;
        faction_ids = ids.factions;
    }

    for (index, layer) in plan.layers.iter().enumerate() {
        // The first plan layer reuses the "Default" layer the map came with.
        let layer_id = match (index, &default_layer) {
            (0, Some(existing)) => {
                // An existing map's first layer may have been hidden or
                // locked by the user; generated objects need it writable.
                if existing.locked {
                    maps::set_layer_locked(&tx, &existing.id, false)?;
                }
                if !existing.visible {
                    maps::set_layer_visibility(&tx, &existing.id, true)?;
                }
                maps::rename_layer(&tx, &existing.id, &layer.name)?.id
            }
            _ => maps::create_layer(&tx, &entity_id, &layer.name)?.id,
        };
        for batch in layer.batches() {
            let ops: Vec<MapOp> = batch
                .iter()
                .map(|object| {
                    // Settlement pins link to their location entity, kingdom
                    // areas to their faction.
                    let entity_id = match object.role {
                        ObjectRole::SettlementPin { index } => {
                            Some(location_ids.get(index).cloned().ok_or_else(|| {
                                AppError::Internal(format!("no location for settlement {index}"))
                            })?)
                        }
                        ObjectRole::Kingdom { index } => {
                            Some(faction_ids.get(index).cloned().ok_or_else(|| {
                                AppError::Internal(format!("no faction for kingdom {index}"))
                            })?)
                        }
                        _ => None,
                    };
                    Ok(MapOp::Create {
                        object: NewMapObject {
                            id: None,
                            layer_id: layer_id.clone(),
                            geometry: object.geometry.clone(),
                            style: Some(object.style.clone()),
                            entity_id,
                            created_at: None,
                        },
                    })
                })
                // Collecting into `AppResult<Vec<_>>` stops at the first
                // error; `?` then returns it.
                .collect::<AppResult<_>>()?;
            maps::apply_ops_in(&tx, &entity_id, ops)?;
            done += 1;
            on_progress(GeneratePhase::Writing, done, total);
        }
    }

    if target.is_some() {
        // An existing map keeps its own tags; `generated` is added to them.
        let mut all = db::tags::names_for_entity(&tx, &entity_id)?;
        all.extend(names.iter().cloned());
        // `normalize_all` also drops duplicates. If the map is already at
        // the tag limit, it keeps the tags it has.
        let all =
            tags::normalize_all(&all).or_else(|_| db::tags::names_for_entity(&tx, &entity_id))?;
        tags::apply_entity_tags(&tx, &entity_id, &all, &mut cache)?;
        db::entities::touch(&tx, &entity_id, &maps::now())?;
    } else {
        tags::apply_entity_tags(&tx, &entity_id, &names, &mut cache)?;
    }
    // The search index has a tags column, so re-index after tagging.
    search::reindex_entity(&tx, &entity_id)?;

    db::world_settings::set(&tx, SETTING_VERSION, &plan.generator_version.to_string())?;
    db::world_settings::set(&tx, SETTING_PARAMS, &serde_json::to_string(&plan.params)?)?;
    db::world_settings::set(&tx, SETTING_LAST_MAP, &entity_id)?;

    tx.commit()?;
    on_progress(GeneratePhase::Writing, total, total);
    Ok(entity_id)
}

/// Creates, tags and fills in one `location` entity per settlement (inside
/// the caller's transaction) and returns their ids in settlement order.
/// `after_each(n)` is called with the number created so far.
fn write_locations(
    conn: &Connection,
    places: &GeneratedPlaces,
    tag_names: &[String],
    cache: &mut std::collections::HashMap<String, String>,
    mut after_each: impl FnMut(usize),
) -> AppResult<Vec<String>> {
    let mut ids = Vec::with_capacity(places.settlements.len());
    for settlement in &places.settlements {
        let entity = entities::create_in(
            conn,
            NewEntity {
                entity_type: "location".into(),
                name: settlement.name.clone(),
            },
        )?;
        tags::apply_entity_tags(conn, &entity.id, tag_names, cache)?;
        // `update_in` validates the properties against the location type's
        // fields and re-indexes the entity for search (tags included).
        entities::update_in(
            conn,
            &entity.id,
            EntityPatch {
                name: None,
                summary: Some(settlement.summary.clone()),
                properties: Some(json!({
                    "kind": settlement.kind,
                    "region": settlement.region,
                })),
            },
        )?;
        ids.push(entity.id);
        after_each(ids.len());
    }
    Ok(ids)
}

/// Entity ids of everything the history part created, by plan index. The
/// `'a` lifetime says it borrows the location ids from the caller.
struct HistoryIds<'a> {
    locations: &'a [String],
    factions: Vec<String>,
    characters: Vec<String>,
    events: Vec<String>,
}

impl HistoryIds<'_> {
    /// The id the writer gave to `r`.
    fn get(&self, r: Ref) -> AppResult<String> {
        let (list, i): (&[String], usize) = match r {
            Ref::Settlement(i) => (self.locations, i),
            Ref::Kingdom(i) => (&self.factions, i),
            Ref::Character(i) => (&self.characters, i),
            Ref::Event(i) => (&self.events, i),
        };
        list.get(i)
            .cloned()
            .ok_or_else(|| AppError::Internal(format!("nothing was created for {r:?}")))
    }
}

/// Absolute day number of a plan date in the display calendar.
fn day_of(calendar: &Calendar, date: HistDate) -> i64 {
    let def = &calendar.definition;
    let start = calendar::year_start_day(def, date.year);
    let length = calendar::days_in_year(def, date.year);
    // `as i64` truncates towards zero; the clamp keeps it inside the year.
    start + ((date.part * length as f64) as i64).clamp(0, length - 1)
}

/// The name of whatever `r` refers to (for plain text).
fn ref_name(history: &GeneratedHistory, places: &GeneratedPlaces, r: Ref) -> String {
    let name = match r {
        Ref::Settlement(i) => places.settlements.get(i).map(|s| &s.name),
        Ref::Kingdom(i) => history.kingdoms.get(i).map(|k| &k.name),
        Ref::Character(i) => history.characters.get(i).map(|c| &c.name),
        Ref::Event(i) => history.events.get(i).map(|e| &e.name),
    };
    name.cloned().unwrap_or_default()
}

/// Creates a tagged entity with a summary and properties; returns its id.
fn create_entity(
    conn: &Connection,
    entity_type: &str,
    name: &str,
    summary: &str,
    properties: Option<serde_json::Value>,
    tag_names: &[String],
    cache: &mut std::collections::HashMap<String, String>,
) -> AppResult<String> {
    let entity = entities::create_in(
        conn,
        NewEntity {
            entity_type: entity_type.into(),
            name: name.into(),
        },
    )?;
    tags::apply_entity_tags(conn, &entity.id, tag_names, cache)?;
    // `update_in` validates the properties against the type's fields and
    // re-indexes the entity for search (tags included).
    entities::update_in(
        conn,
        &entity.id,
        EntityPatch {
            name: None,
            summary: Some(summary.to_owned()),
            properties,
        },
    )?;
    Ok(entity.id)
}

/// Writes the history part of a plan (inside the caller's transaction):
/// faction and character entities, events with their participants,
/// relationships, then the lore documents (last, when every mention has an
/// id). `step` is called once per group.
fn write_history<'a>(
    conn: &Connection,
    history: &GeneratedHistory,
    places: &GeneratedPlaces,
    locations: &'a [String],
    tag_names: &[String],
    cache: &mut std::collections::HashMap<String, String>,
    step: &mut dyn FnMut(),
) -> AppResult<HistoryIds<'a>> {
    let mut ids = HistoryIds {
        locations,
        factions: Vec::with_capacity(history.kingdoms.len()),
        characters: Vec::with_capacity(history.characters.len()),
        events: Vec::with_capacity(history.events.len()),
    };

    for kingdom in &history.kingdoms {
        let leader = history
            .characters
            .get(kingdom.ruler)
            .map(|c| c.name.clone())
            .unwrap_or_default();
        let id = create_entity(
            conn,
            "faction",
            &kingdom.name,
            &kingdom.summary,
            Some(json!({ "kind": "kingdom", "leader": leader })),
            tag_names,
            cache,
        )?;
        ids.factions.push(id);
    }
    step();

    for person in &history.characters {
        let mut properties = json!({
            "species": person.species,
            "status": if person.alive { "alive" } else { "dead" },
        });
        if let Some(age) = person.age {
            properties["age"] = json!(age);
        }
        let id = create_entity(
            conn,
            "character",
            &person.name,
            &person.summary,
            Some(properties),
            tag_names,
            cache,
        )?;
        ids.characters.push(id);
    }
    step();

    // Dates go in as day numbers of the display calendar: the year length
    // (and leap years) come from its definition.
    let display = calendar_manager::get_display(conn)?;
    for event in &history.events {
        let start = day_of(&display, event.start);
        let end = event.end.map(|d| day_of(&display, d));
        let created =
            events::create_in(conn, &event.name, start, end, event.importance, &display.id)?;
        tags::apply_entity_tags(conn, &created.entity_id, tag_names, cache)?;
        entities::update_in(
            conn,
            &created.entity_id,
            EntityPatch {
                name: None,
                summary: Some(event.summary.clone()),
                properties: None,
            },
        )?;
        ids.events.push(created.entity_id);
    }
    for (event, event_id) in history.events.iter().zip(&ids.events) {
        for who in &event.participants {
            events::add_involved(conn, event_id, &ids.get(*who)?)?;
        }
    }
    step();

    for rel in &history.relationships {
        relationships::create(
            conn,
            &ids.get(rel.source)?,
            &ids.get(rel.target)?,
            rel.kind,
            rel.label.clone(),
        )?;
    }
    step();

    for doc in &history.documents {
        let target = ids.get(doc.target)?;
        // Mentions get the ids created above; years are written in the
        // display calendar (e.g. "412 AF").
        let (content, plain) = doc.render(
            |r| ids.get(r),
            |r| ref_name(history, places, r),
            |d| {
                calendar::format(&display.definition, day_of(&display, d), DateStyle::Year)
                    .unwrap_or_else(|_| format!("year {}", d.year + 1))
            },
        )?;
        // `documents::save_in` also records the mention links and updates
        // the search index.
        documents::save_in(conn, &target, content.to_string(), plain)?;
    }
    step();
    Ok(ids)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::error::AppError;
    use crate::models::generator::GenerationScope;
    use crate::models::generator::GeneratorParams;
    use crate::models::map::{MapGeometry, Point};
    use crate::services::generator::{
        GeneratedObject, LAYER_FEATURES, LAYER_PLACES, LAYER_RIVERS, LAYER_ROADS, LAYER_TERRAIN,
        ObjectRole, generate_map,
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
            None,
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
        let id = write_map(
            &w.conn,
            &assets_dir,
            plan,
            "Generated map 1",
            None,
            &mut record,
        )
        .unwrap();

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
        let result = write_map(&w.conn, &not_a_dir, plan(), "Map", None, &mut quiet);
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

    /// A small plan with places (settlements, roads, pins).
    fn places_plan() -> &'static GeneratedMap {
        static PLAN: OnceLock<GeneratedMap> = OnceLock::new();
        PLAN.get_or_init(|| {
            generate_map(&GeneratorParams {
                seed: 1,
                width: 800,
                height: 600,
                scope: GenerationScope::Places,
                ..GeneratorParams::default()
            })
            .unwrap()
        })
    }

    #[test]
    fn writes_places_as_linked_locations() {
        let tmp = tempfile::tempdir().unwrap();
        let w = new_world(tmp.path());
        let assets_dir = w.path.join(ASSETS_DIR);
        let plan = places_plan();
        let settlements = &plan.places.as_ref().unwrap().settlements;
        assert!(!settlements.is_empty());

        let mut last = (GeneratePhase::Rendering, 0, 0);
        let mut record = |p: GeneratePhase, d: u32, t: u32| last = (p, d, t);
        let id = write_map(&w.conn, &assets_dir, plan, "Places", None, &mut record).unwrap();
        assert_eq!(last.0, GeneratePhase::Writing);
        assert_eq!(last.1, last.2);

        let locations = entities::list(&w.conn, Some("location")).unwrap();
        assert_eq!(locations.len(), settlements.len());
        assert_eq!(plan.stats.settlements as usize, settlements.len());

        let data = maps::get_map(&w.conn, &assets_dir, &id).unwrap();
        let names: Vec<&str> = data.layers.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(
            names,
            [
                LAYER_TERRAIN,
                LAYER_RIVERS,
                LAYER_ROADS,
                LAYER_FEATURES,
                LAYER_PLACES
            ]
        );
        let layer_id = |name: &str| {
            data.layers
                .iter()
                .find(|l| l.name == name)
                .unwrap()
                .id
                .clone()
        };
        let roads = layer_id(LAYER_ROADS);
        let road_count = data.objects.iter().filter(|o| o.layer_id == roads).count();
        assert_eq!(road_count, plan.stats.roads as usize);

        let places_layer = layer_id(LAYER_PLACES);
        let pins: Vec<_> = data
            .objects
            .iter()
            .filter(|o| o.layer_id == places_layer && matches!(o.geometry, MapGeometry::Pin { .. }))
            .collect();
        assert_eq!(pins.len(), settlements.len());
        for (pin, s) in pins.iter().zip(settlements) {
            // Same order as the plan, each pin linked to its own location.
            let entity_id = pin.entity_id.as_deref().expect("pin is linked");
            assert_eq!(pin.geometry, MapGeometry::Pin { x: s.x, y: s.y });
            assert_eq!(pin.entity_type.as_deref(), Some("location"));
            let entity = entities::get(&w.conn, entity_id).unwrap();
            assert_eq!(entity.name, s.name);
            assert_eq!(entity.properties["kind"], s.kind);
            assert_eq!(entity.properties["region"], s.region.as_str());
            assert_eq!(entity.summary, s.summary);
            assert_eq!(entity.tags, [TAG_GENERATED]);
        }
        let linked: std::collections::HashSet<&str> =
            pins.iter().filter_map(|p| p.entity_id.as_deref()).collect();
        assert_eq!(linked.len(), settlements.len());

        // Search finds a settlement by name.
        let hits = search::search(&w.conn, &settlements[0].name, 10).unwrap();
        assert!(
            hits.iter().any(|h| h.name == settlements[0].name),
            "{hits:?}"
        );
    }

    #[test]
    fn a_failure_after_places_leaves_no_locations_or_pins() {
        let tmp = tempfile::tempdir().unwrap();
        let w = new_world(tmp.path());
        let before = count(&w.conn, "entities");
        // The bad object is on the last layer, after every location entity
        // and pin was written.
        let mut bad = places_plan().clone();
        let last = bad.layers.len() - 1;
        bad.layers[last].objects.push(GeneratedObject {
            geometry: MapGeometry::Area {
                points: vec![Point { x: 1.0, y: 1.0 }, Point { x: 2.0, y: 2.0 }],
            },
            style: json!({}),
            role: ObjectRole::Coast,
        });
        let err = write(&w, &bad).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)), "{err:?}");
        assert_eq!(count(&w.conn, "entities"), before);
        assert!(
            entities::list(&w.conn, Some("location"))
                .unwrap()
                .is_empty()
        );
        assert_eq!(count(&w.conn, "map_objects"), 0);
        assert_eq!(png_files(&w.path.join(ASSETS_DIR)), 0);
    }
    /// A new, empty `map` entity (no background, one default layer).
    fn empty_map(w: &WorldHandle, name: &str) -> String {
        entities::create(
            &w.conn,
            NewEntity {
                entity_type: "map".into(),
                name: name.into(),
            },
        )
        .unwrap()
        .id
    }

    fn write_into(w: &WorldHandle, plan: &GeneratedMap, target: &str) -> AppResult<String> {
        let mut quiet = |_: GeneratePhase, _: u32, _: u32| {};
        write_map(
            &w.conn,
            &w.path.join(ASSETS_DIR),
            plan,
            "ignored name",
            Some(target),
            &mut quiet,
        )
    }

    #[test]
    fn writes_into_an_existing_empty_map() {
        let tmp = tempfile::tempdir().unwrap();
        let w = new_world(tmp.path());
        let assets_dir = w.path.join(ASSETS_DIR);
        let target = empty_map(&w, "Old Kingdom");
        tags::set_entity_tags(&w.conn, &target, &["realm".to_owned()]).unwrap();
        let maps_before = count(&w.conn, "maps");
        let entities_before = count(&w.conn, "entities");
        let plan = places_plan();

        let id = write_into(&w, plan, &target).unwrap();
        assert_eq!(id, target);
        // No new map entity; only the settlement locations were added.
        assert_eq!(count(&w.conn, "maps"), maps_before);
        let settlements = plan.places.as_ref().unwrap().settlements.len() as i64;
        assert_eq!(count(&w.conn, "entities"), entities_before + settlements);

        let data = maps::get_map(&w.conn, &assets_dir, &id).unwrap();
        assert_eq!(data.map.name, "Old Kingdom");
        let bg = data.map.background.as_ref().expect("background set");
        assert!(Path::new(&bg.path).is_file());
        assert_eq!((data.map.width, data.map.height), (800, 600));
        let names: Vec<&str> = data.layers.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(
            names,
            [
                LAYER_TERRAIN,
                LAYER_RIVERS,
                LAYER_ROADS,
                LAYER_FEATURES,
                LAYER_PLACES
            ]
        );
        let planned: usize = plan.layers.iter().map(|l| l.objects.len()).sum();
        assert_eq!(data.objects.len(), planned);

        // Same result as writing a fresh map from the same plan.
        let fresh = write(&w, plan).unwrap();
        let fresh_data = maps::get_map(&w.conn, &assets_dir, &fresh).unwrap();
        assert_eq!(fresh_data.objects.len(), data.objects.len());
        let geometries = |d: &crate::models::map::MapData| -> Vec<MapGeometry> {
            d.objects.iter().map(|o| o.geometry.clone()).collect()
        };
        assert_eq!(geometries(&fresh_data), geometries(&data));

        let mut tag_names = db::tags::names_for_entity(&w.conn, &id).unwrap();
        tag_names.sort();
        assert_eq!(tag_names, [TAG_GENERATED, "realm"]);
        let last = db::world_settings::get(&w.conn, SETTING_LAST_MAP).unwrap();
        assert_eq!(last.as_deref(), Some(fresh.as_str()));
    }

    #[test]
    fn a_map_without_layers_gets_a_terrain_layer() {
        let tmp = tempfile::tempdir().unwrap();
        let w = new_world(tmp.path());
        let target = empty_map(&w, "Bare");
        let info = db::maps::get_map_by_entity(&w.conn, &target)
            .unwrap()
            .unwrap();
        for layer in db::maps::list_layers(&w.conn, &info.id).unwrap() {
            db::maps::delete_layer(&w.conn, &layer.id).unwrap();
        }
        write_into(&w, plan(), &target).unwrap();
        let data = maps::get_map(&w.conn, &w.path.join(ASSETS_DIR), &target).unwrap();
        let names: Vec<&str> = data.layers.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, [LAYER_TERRAIN, LAYER_RIVERS, LAYER_FEATURES]);
        let setting = db::world_settings::get(&w.conn, SETTING_LAST_MAP).unwrap();
        assert_eq!(setting.as_deref(), Some(target.as_str()));
    }

    #[test]
    fn refuses_a_target_with_content() {
        let tmp = tempfile::tempdir().unwrap();
        let w = new_world(tmp.path());
        let assets_dir = w.path.join(ASSETS_DIR);

        // A map with an object on it.
        let with_object = empty_map(&w, "Drawn");
        let layer_id = maps::get_map(&w.conn, &assets_dir, &with_object)
            .unwrap()
            .layers[0]
            .id
            .clone();
        maps::create_object(
            &w.conn,
            NewMapObject {
                id: None,
                layer_id,
                geometry: MapGeometry::Pin { x: 5.0, y: 5.0 },
                style: None,
                entity_id: None,
                created_at: None,
            },
        )
        .unwrap();

        // A map with a background image (from an earlier generation).
        let with_image = write(&w, plan()).unwrap();

        // Not a map at all.
        let character = entities::create(
            &w.conn,
            NewEntity {
                entity_type: "character".into(),
                name: "Ana".into(),
            },
        )
        .unwrap()
        .id;

        let before = (
            count(&w.conn, "entities"),
            count(&w.conn, "map_layers"),
            count(&w.conn, "map_objects"),
            count(&w.conn, "assets"),
            png_files(&assets_dir),
        );
        for target in [&with_object, &with_image] {
            let err = write_into(&w, places_plan(), target).unwrap_err();
            match err {
                AppError::Validation(msg) => assert_eq!(msg, TARGET_NOT_EMPTY),
                other => panic!("{other:?}"),
            }
        }
        assert!(matches!(
            write_into(&w, plan(), &character),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            write_into(&w, plan(), "missing"),
            Err(AppError::NotFound(_))
        ));
        let after = (
            count(&w.conn, "entities"),
            count(&w.conn, "map_layers"),
            count(&w.conn, "map_objects"),
            count(&w.conn, "assets"),
            png_files(&assets_dir),
        );
        assert_eq!(before, after);
        assert!(
            db::tags::names_for_entity(&w.conn, &with_object)
                .unwrap()
                .is_empty()
        );
        let drawn = maps::get_map(&w.conn, &assets_dir, &with_object).unwrap();
        assert!(drawn.map.background.is_none());
        assert_eq!(
            drawn.layers[0].name,
            crate::services::maps::DEFAULT_LAYER_NAME
        );
    }

    /// A small plan with the full history scope.
    fn history_plan() -> &'static GeneratedMap {
        static PLAN: OnceLock<GeneratedMap> = OnceLock::new();
        PLAN.get_or_init(|| {
            generate_map(&GeneratorParams {
                seed: 1,
                width: 800,
                height: 600,
                scope: GenerationScope::History,
                ..GeneratorParams::default()
            })
            .unwrap()
        })
    }

    fn count_kind(conn: &Connection, kind: &str) -> i64 {
        conn.query_row("SELECT count(*) FROM links WHERE kind = ?1", [kind], |r| {
            r.get(0)
        })
        .unwrap()
    }

    /// The id of the only entity of `entity_type` called `name`.
    fn id_named(conn: &Connection, entity_type: &str, name: &str) -> String {
        let found: Vec<_> = entities::list(conn, Some(entity_type))
            .unwrap()
            .into_iter()
            .filter(|e| e.name == name)
            .collect();
        assert_eq!(found.len(), 1, "{entity_type} {name}");
        found[0].id.clone()
    }

    #[test]
    fn writes_history_as_linked_entities() {
        use crate::services::generator::{EventKind, LAYER_KINGDOMS, PersonRole};
        use crate::services::{graph, links};

        let tmp = tempfile::tempdir().unwrap();
        let w = new_world(tmp.path());
        let assets_dir = w.path.join(ASSETS_DIR);
        let plan = history_plan();
        let h = plan.history.as_ref().unwrap();
        let settlements = &plan.places.as_ref().unwrap().settlements;

        let mut last = (GeneratePhase::Rendering, 0, 0);
        let mut record = |p: GeneratePhase, d: u32, t: u32| last = (p, d, t);
        let map_id = write_map(&w.conn, &assets_dir, plan, "World", None, &mut record).unwrap();
        assert_eq!(last.0, GeneratePhase::Writing);
        assert_eq!(last.1, last.2);

        let factions = entities::list(&w.conn, Some("faction")).unwrap();
        let characters = entities::list(&w.conn, Some("character")).unwrap();
        let events_list = entities::list(&w.conn, Some("event")).unwrap();
        assert_eq!(factions.len(), h.kingdoms.len());
        assert_eq!(characters.len(), h.characters.len());
        assert_eq!(events_list.len(), h.events.len());
        assert_eq!(count(&w.conn, "documents") as usize, h.documents.len());
        assert_eq!(plan.stats.kingdoms as usize, h.kingdoms.len());
        assert_eq!(plan.stats.documents as usize, h.documents.len());

        // Every participant is an `involved_in` link; every relationship a
        // row; every lore stub records its mentions.
        let involved: usize = h.events.iter().map(|e| e.participants.len()).sum();
        assert_eq!(count_kind(&w.conn, "involved_in") as usize, involved);
        let rels: i64 = relationships::list_kinds()
            .iter()
            .map(|k| count_kind(&w.conn, &k.key))
            .sum();
        assert_eq!(rels as usize, h.relationships.len());
        assert!(count_kind(&w.conn, "mention") > 0);

        for (k, kingdom) in h.kingdoms.iter().enumerate() {
            let faction_id = id_named(&w.conn, "faction", &kingdom.name);
            let faction = entities::get(&w.conn, &faction_id).unwrap();
            let ruler = &h.characters[kingdom.ruler];
            assert_eq!(faction.properties["kind"], "kingdom");
            assert_eq!(faction.properties["leader"], ruler.name.as_str());
            assert_eq!(faction.summary, kingdom.summary);
            assert_eq!(faction.tags, [TAG_GENERATED]);

            // The capital's backlinks include its kingdom's lore.
            let capital = &settlements[kingdom.capital];
            let capital_id = id_named(&w.conn, "location", &capital.name);
            let back = links::backlinks(&w.conn, &capital_id, &capital.name).unwrap();
            assert!(back.iter().any(|b| b.source_id == faction_id), "{back:?}");
            let lore = documents::get(&w.conn, &faction_id).unwrap();
            assert!(lore.plain_text.contains(&capital.name));
            assert!(lore.content.contains(&capital_id));
            // Search finds the kingdom by name.
            let hits = search::search(&w.conn, &kingdom.name, 100).unwrap();
            assert!(
                hits.iter().any(|hit| hit.entity_id == faction_id),
                "{hits:?}"
            );

            // The family tree around the ruler shows the founder
            // (grandparent) and the children.
            let ruler_id = id_named(&w.conn, "character", &ruler.name);
            let ruler_entity = entities::get(&w.conn, &ruler_id).unwrap();
            assert_eq!(ruler_entity.properties["status"], "alive");
            assert_eq!(ruler_entity.properties["age"], ruler.age.unwrap());
            let tree = graph::get_family_tree(&w.conn, &ruler_id, None, None).unwrap();
            let generation_of = |name: &str| {
                tree.nodes
                    .iter()
                    .find(|n| n.name == name)
                    .map(|n| n.generation)
            };
            for p in h.characters.iter().filter(|p| p.kingdom == k) {
                let expected = match p.role {
                    PersonRole::Founder => -2,
                    PersonRole::Heir => -1,
                    PersonRole::Ruler | PersonRole::Spouse => 0,
                    PersonRole::Child => 1,
                };
                assert_eq!(generation_of(&p.name), Some(expected), "{}", p.name);
                if p.role == PersonRole::Founder {
                    let founder =
                        entities::get(&w.conn, &id_named(&w.conn, "character", &p.name)).unwrap();
                    assert_eq!(founder.properties["status"], "dead");
                }
            }

            // The kingdom's wars and coronation are on its timeline.
            let timeline = events::list_for_entity(&w.conn, &faction_id).unwrap();
            let planned = h
                .events
                .iter()
                .filter(|e| e.participants.contains(&Ref::Kingdom(k)))
                .count();
            assert_eq!(timeline.len(), planned);
        }

        // Event dates are day numbers of the display calendar, in order.
        let display = calendar_manager::get_display(&w.conn).unwrap();
        let mut previous = i64::MIN;
        for e in &h.events {
            let id = id_named(&w.conn, "event", &e.name);
            let stored = events::get(&w.conn, &id).unwrap();
            assert_eq!(stored.start_day, day_of(&display, e.start));
            assert_eq!(stored.end_day, e.end.map(|d| day_of(&display, d)));
            assert_eq!(stored.importance, e.importance);
            assert_eq!(stored.calendar_id, display.id);
            assert!(stored.start_day >= previous);
            previous = stored.start_day;
            if e.kind == EventKind::War {
                assert!(stored.end_day.unwrap() > stored.start_day);
            }
        }

        // Kingdom areas sit above the terrain and link to their factions.
        let data = maps::get_map(&w.conn, &assets_dir, &map_id).unwrap();
        let names: Vec<&str> = data.layers.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(
            names,
            [
                LAYER_TERRAIN,
                LAYER_KINGDOMS,
                LAYER_RIVERS,
                LAYER_ROADS,
                LAYER_FEATURES,
                LAYER_PLACES
            ]
        );
        let kingdoms_layer = &data.layers[1].id;
        let areas: Vec<_> = data
            .objects
            .iter()
            .filter(|o| &o.layer_id == kingdoms_layer)
            .collect();
        assert!(!areas.is_empty());
        for area in areas {
            assert_eq!(area.entity_type.as_deref(), Some("faction"));
        }
    }

    #[test]
    fn a_failure_after_history_leaves_no_history_rows() {
        let tmp = tempfile::tempdir().unwrap();
        let w = new_world(tmp.path());
        let before = (
            count(&w.conn, "entities"),
            count(&w.conn, "links"),
            count(&w.conn, "documents"),
            count(&w.conn, "events"),
        );
        // The bad object is on the last layer: written after every faction,
        // character, event, relationship and document.
        let mut bad = history_plan().clone();
        let last = bad.layers.len() - 1;
        bad.layers[last].objects.push(GeneratedObject {
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
            count(&w.conn, "links"),
            count(&w.conn, "documents"),
            count(&w.conn, "events"),
        );
        assert_eq!(before, after);
        for t in ["faction", "character", "event", "location"] {
            assert!(entities::list(&w.conn, Some(t)).unwrap().is_empty(), "{t}");
        }
        assert_eq!(png_files(&w.path.join(ASSETS_DIR)), 0);

        // A broken reference inside the history part fails just the same.
        let mut bad = history_plan().clone();
        bad.history
            .as_mut()
            .unwrap()
            .relationships
            .push(crate::services::generator::Rel {
                source: Ref::Kingdom(0),
                target: Ref::Kingdom(999),
                kind: "ally_of",
                label: None,
            });
        assert!(matches!(write(&w, &bad), Err(AppError::Internal(_))));
        assert_eq!(count(&w.conn, "entities"), before.0);
        assert_eq!(count(&w.conn, "documents"), before.2);
    }
}
