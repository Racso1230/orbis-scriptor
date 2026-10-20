//! Procedural terrain generator (Phase 14a).
//!
//! Pure and deterministic: `generate_map` takes params and returns an
//! in-memory [`GeneratedMap`] plan. It touches no database or files; the
//! same params always give exactly the same plan (geometry and PNG bytes).
//!
//! Pipeline:
//! 1. `fields`  — height (fBm noise × land-shape mask), sea level from the
//!    land-% quantile, moisture, temperature, biomes on a coarse grid
//! 2. `rivers`  — drainage + flow accumulation, traced to the sea
//! 3. `coast`   — marching squares → simplified land and lake polygons
//! 4. `scatter` — Poisson-disk biome stamps
//! 5. `render`  — background PNG
//! 6. `places`  — (scope "places") settlements, names (`names`) and roads
//!    (`roads`, A*)
//! 7. `politics`, `history`, `lore` — (scope "history") kingdoms grown from
//!    the capitals, dynasties, relationships, timeline events and lore
//! 8. `plan`    — Terrain / Kingdoms / Rivers / Roads / Features / Places
//!    layers + stats

mod biomes;
mod climate;
pub mod coast;
pub mod fields;
mod geometry;
pub mod history;
mod land_shape;
pub mod lore;
pub mod names;
pub mod places;
pub mod plan;
pub mod politics;
pub mod render;
pub mod rivers;
mod rng;
pub mod roads;
pub mod scatter;
pub mod stamps;
pub mod writer;

use std::collections::BTreeMap;

use crate::error::{AppError, AppResult};
use crate::models::generate::GeneratePhase;
use crate::models::generator::{GenerationScope, GenerationStats, GeneratorParams};

pub use fields::{GridLayout, TerrainGrid};
pub use places::{RegionName, Settlement, Tier};
pub use plan::{
    EventKind, GENERATOR_VERSION, GeneratedHistory, GeneratedLayer, GeneratedMap, GeneratedObject,
    GeneratedPlaces, HistDate, HistEvent, Kingdom, LAYER_FEATURES, LAYER_KINGDOMS, LAYER_PLACES,
    LAYER_RIVERS, LAYER_ROADS, LAYER_TERRAIN, LoreDoc, LoreInline, ObjectRole, PRESENT_YEAR,
    Person, PersonRole, Ref, Rel,
};
pub use roads::{Road, Route};
pub use stamps::{is_known_stamp, known_stamp_keys};

/// Generates the full plan for a map (terrain, and places when the scope
/// asks for them).
pub fn generate_map(params: &GeneratorParams) -> AppResult<GeneratedMap> {
    generate_map_with_progress(params, &mut |_, _, _| {})
}

/// [`generate_map`] that reports its stages: `(generating, 0, 1)` …
/// `(generating, 1, 1)`, then for the places and history scopes
/// `(placing, 0, 1)` … `(placing, 1, 1)`, and for the history scope
/// `(history, 0, 1)` … `(history, 1, 1)`. Still pure: the callback is the
/// only side effect.
pub fn generate_map_with_progress(
    params: &GeneratorParams,
    on_progress: &mut dyn FnMut(GeneratePhase, u32, u32),
) -> AppResult<GeneratedMap> {
    params.validate()?;
    on_progress(GeneratePhase::Generating, 0, 1);
    let layout = GridLayout::for_map(params.width, params.height);
    let mut grid = fields::build_grid(params, layout);

    let mut river_rng = rng::stage_rng(params.seed, rng::Stage::Rivers);
    let rivers = rivers::trace_rivers(&mut grid, &mut river_rng);
    let polygons = coast::coast_polygons(&grid);
    let mut scatter_rng = rng::stage_rng(params.seed, rng::Stage::Scatter);
    let mut stamps = scatter::scatter_stamps(&grid, params.climate, &mut scatter_rng);

    let (bw, bh) = render::background_size(params.width, params.height);
    let image = render::render_image(&grid, params.climate, bw, bh);
    let background_png = render::encode_png(&image)?;
    on_progress(GeneratePhase::Generating, 1, 1);

    let mut places = None;
    let mut history = None;
    let mut kingdom_layer = None;
    if params.scope != GenerationScope::MapOnly {
        on_progress(GeneratePhase::Placing, 0, 1);
        let (mut p, mut names) = generate_places(&grid, params);
        clear_stamps_around(&grid, &mut stamps, &p.settlements);
        on_progress(GeneratePhase::Placing, 1, 1);
        if params.scope == GenerationScope::History {
            on_progress(GeneratePhase::History, 0, 1);
            let (h, layer) = generate_history(&grid, params, &mut p, &mut names);
            history = Some(h);
            kingdom_layer = Some(layer);
            on_progress(GeneratePhase::History, 1, 1);
        }
        places = Some(p);
    }

    let mut stats = stats(&grid, rivers.len(), stamps.len());
    let mut layers = vec![plan::terrain_layer(&grid, params.climate, polygons)];
    // Kingdoms sit just above the terrain, below rivers, roads and places.
    if let Some(layer) = kingdom_layer {
        layers.push(layer);
    }
    layers.push(plan::river_layer(&grid, params.climate, &rivers));
    // `if let Some(p) = &places` borrows the value inside the Option
    // without moving it out, so `places` can still go into the plan below.
    if let Some(p) = &places {
        layers.push(plan::road_layer(&grid, &p.roads));
    }
    layers.push(plan::feature_layer(&stamps));
    if let Some(p) = &places {
        layers.push(plan::places_layer(&grid, &p.settlements));
        stats.settlements = p.settlements.len() as u32;
        stats.roads = p.roads.len() as u32;
    }
    if let Some(h) = &history {
        stats.kingdoms = h.kingdoms.len() as u32;
        stats.characters = h.characters.len() as u32;
        stats.events = h.events.len() as u32;
        stats.documents = h.documents.len() as u32;
    }
    Ok(GeneratedMap {
        params: params.clone(),
        generator_version: GENERATOR_VERSION,
        width: params.width,
        height: params.height,
        background_png,
        background_width: bw,
        background_height: bh,
        layers,
        stats,
        grid,
        rivers,
        places,
        history,
    })
}

/// Settlements, their names and regions, and the roads between them. Also
/// returns the name generator, so the history stage can keep every name in
/// the world unique.
fn generate_places(
    grid: &TerrainGrid,
    params: &GeneratorParams,
) -> (GeneratedPlaces, names::NameGenerator) {
    let mut place_rng = rng::stage_rng(params.seed, rng::Stage::Places);
    let mut names = names::NameGenerator::new(rng::stage_rng(params.seed, rng::Stage::Names));
    let (settlements, regions, spacing) =
        places::place_settlements(grid, params, &mut place_rng, &mut names);
    let (roads, routes) = roads::build_roads(grid, &settlements);
    let places = GeneratedPlaces {
        settlements,
        roads,
        routes,
        regions,
        spacing,
    };
    (places, names)
}

/// Kingdoms, dynasties, relationships, events and lore (scope "history"),
/// plus the Kingdoms layer. Updates the settlements' `kingdom` and summary.
/// Each part draws from its own random stream (see `rng::Stage`).
fn generate_history(
    grid: &TerrainGrid,
    params: &GeneratorParams,
    places: &mut GeneratedPlaces,
    names: &mut names::NameGenerator,
) -> (GeneratedHistory, GeneratedLayer) {
    let mut politics_rng = rng::stage_rng(params.seed, rng::Stage::Politics);
    let realms = politics::build_realms(grid, &places.settlements, &mut politics_rng);
    names.reseed(rng::stage_rng(params.seed, rng::Stage::HistoryNames));
    let mut history_rng = rng::stage_rng(params.seed, rng::Stage::History);
    let chronicle = history::build(places, &realms, names, &mut history_rng);
    let mut lore_rng = rng::stage_rng(params.seed, rng::Stage::Lore);
    let documents = lore::build_documents(&chronicle, places, &mut lore_rng);
    let layer = politics::kingdom_layer(grid, &realms);
    let history = GeneratedHistory {
        kingdoms: chronicle.kingdoms,
        characters: chronicle.characters,
        relationships: chronicle.relationships,
        events: chronicle.events,
        documents,
        territory: realms.owner,
    };
    (history, layer)
}

/// Removes the biome stamps a settlement stamp would sit on.
fn clear_stamps_around(
    grid: &TerrainGrid,
    stamps: &mut Vec<scatter::StampPlacement>,
    settlements: &[Settlement],
) {
    let size = places::stamp_size(grid);
    // `retain` keeps only the elements for which the closure returns true.
    stamps.retain(|st| {
        settlements.iter().all(|s| {
            let reach = 32.0 * (size * s.tier.stamp_factor() + st.scale) * 0.9;
            (st.x - s.x).hypot(st.y - s.y) >= reach
        })
    });
}

/// Smallest and largest preview side the wizard may ask for.
pub const PREVIEW_SIDE: std::ops::RangeInclusive<u32> = 64..=2048;

/// A quick low-resolution PNG of the terrain for the wizard. The longer
/// side is `max_side` pixels. Uses a coarser grid and skips rivers, vector
/// tracing and stamps, so it takes a fraction of a full generation.
pub fn render_preview(params: &GeneratorParams, max_side: u32) -> AppResult<Vec<u8>> {
    params.validate()?;
    if !PREVIEW_SIDE.contains(&max_side) {
        return Err(AppError::Validation(format!(
            "Preview size must be between {} and {} pixels",
            PREVIEW_SIDE.start(),
            PREVIEW_SIDE.end()
        )));
    }
    let longest = params.width.max(params.height);
    let (w, h) = if params.width >= params.height {
        let h = (u64::from(params.height) * u64::from(max_side) / u64::from(longest)) as u32;
        (max_side, h.max(1))
    } else {
        let w = (u64::from(params.width) * u64::from(max_side) / u64::from(longest)) as u32;
        (w.max(1), max_side)
    };
    // One grid node per two preview pixels, but never finer than the full
    // generation (so a small map's preview matches its real terrain).
    let full = GridLayout::for_map(params.width, params.height);
    let cell = (full.cell as u32).max(longest.div_ceil((max_side / 2).max(1)));
    let layout = GridLayout::with_cell(params.width, params.height, cell);
    let grid = fields::build_grid(params, layout);
    let image = render::render_image(&grid, params.climate, w, h);
    render::encode_png(&image)
}

fn stats(grid: &TerrainGrid, rivers: usize, stamps: usize) -> GenerationStats {
    let min_cells = coast::min_island_cells(grid);
    let masses = coast::land_components(grid)
        .into_iter()
        .filter(|s| *s >= min_cells)
        .count();
    let mut biome_counts = BTreeMap::new();
    for b in &grid.biome {
        // `entry(..).or_insert(0)` gives a mutable reference to the count,
        // inserting 0 first if this biome has not been seen yet.
        *biome_counts.entry(*b).or_insert(0u32) += 1;
    }
    GenerationStats {
        land_percent_actual: (grid.land_percent() * 10.0).round() / 10.0,
        land_masses: masses as u32,
        rivers: rivers as u32,
        stamps: stamps as u32,
        biome_counts,
        settlements: 0,
        roads: 0,
        kingdoms: 0,
        characters: 0,
        events: 0,
        documents: 0,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::models::generator::{Biome, Climate, LandShape};
    use crate::models::map::MapGeometry;
    use crate::services::maps::{MAX_OPS, MAX_POINTS, MAX_STYLE_BYTES};

    fn small(seed: u32) -> GeneratorParams {
        GeneratorParams {
            seed,
            width: 800,
            height: 600,
            ..GeneratorParams::default()
        }
    }

    fn layer<'a>(map: &'a GeneratedMap, name: &str) -> &'a GeneratedLayer {
        map.layers.iter().find(|l| l.name == name).unwrap()
    }

    #[test]
    fn same_params_give_identical_plan() {
        let a = generate_map(&small(42)).unwrap();
        let b = generate_map(&small(42)).unwrap();
        assert_eq!(a.layers, b.layers);
        assert_eq!(a.background_png, b.background_png);
        assert_eq!(a.stats, b.stats);
    }

    #[test]
    fn different_seeds_differ() {
        let a = generate_map(&small(1)).unwrap();
        let b = generate_map(&small(2)).unwrap();
        assert_ne!(a.layers, b.layers);
        assert_ne!(a.background_png, b.background_png);
    }

    #[test]
    fn layers_have_expected_names_and_content() {
        let map = generate_map(&small(42)).unwrap();
        let names: Vec<&str> = map.layers.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, [LAYER_TERRAIN, LAYER_RIVERS, LAYER_FEATURES]);
        assert!(!layer(&map, LAYER_TERRAIN).objects.is_empty());
        assert!(!layer(&map, LAYER_RIVERS).objects.is_empty());
        assert!(!layer(&map, LAYER_FEATURES).objects.is_empty());
        assert_eq!(
            map.stats.rivers as usize,
            layer(&map, LAYER_RIVERS).objects.len()
        );
        assert_eq!(
            map.stats.stamps as usize,
            layer(&map, LAYER_FEATURES).objects.len()
        );
        let png = image::load_from_memory(&map.background_png).unwrap();
        assert_eq!((png.width(), png.height()), (800, 600));
    }

    #[test]
    fn objects_respect_map_service_limits() {
        for shape in [
            LandShape::Continent,
            LandShape::Archipelago,
            LandShape::Coastline,
        ] {
            let map = generate_map(&GeneratorParams {
                land_shape: shape,
                scope: GenerationScope::Places,
                ..small(7)
            })
            .unwrap();
            for layer in &map.layers {
                for batch in layer.batches() {
                    assert!(batch.len() <= MAX_OPS);
                }
                for obj in &layer.objects {
                    assert!(obj.style.is_object());
                    assert!(obj.style.to_string().len() <= MAX_STYLE_BYTES);
                    match &obj.geometry {
                        MapGeometry::Area { points } => {
                            assert!((3..=MAX_POINTS).contains(&points.len()));
                        }
                        MapGeometry::Path { points, .. } => {
                            assert!((2..=MAX_POINTS).contains(&points.len()));
                        }
                        MapGeometry::Stamp { scale, .. } => assert!(*scale > 0.0),
                        MapGeometry::Pin { .. } => {}
                        other => panic!("unexpected geometry {other:?}"),
                    }
                    let in_map =
                        |x: f64, y: f64| (0.0..=800.0).contains(&x) && (0.0..=600.0).contains(&y);
                    match &obj.geometry {
                        MapGeometry::Area { points } | MapGeometry::Path { points, .. } => {
                            assert!(points.iter().all(|p| in_map(p.x, p.y)));
                        }
                        MapGeometry::Stamp { x, y, .. } | MapGeometry::Pin { x, y } => {
                            assert!(in_map(*x, *y));
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    #[test]
    fn rivers_end_in_water_or_join_their_parent() {
        for seed in [3, 42, 99] {
            let map = generate_map(&small(seed)).unwrap();
            assert!(!map.rivers.is_empty());
            for river in &map.rivers {
                let mouth = *river.nodes.last().unwrap();
                match river.parent {
                    None => {
                        assert!(map.grid.height[mouth] <= map.grid.sea_level);
                    }
                    Some(p) => assert!(map.rivers[p].nodes.contains(&mouth)),
                }
                // Rivers only ever flow downhill or level (pits are flooded).
                let source = river.nodes[0];
                assert!(map.grid.height[source] >= map.grid.height[mouth]);
            }
        }
    }

    #[test]
    fn only_known_stamp_keys_are_used() {
        for climate in [
            Climate::Temperate,
            Climate::Frozen,
            Climate::Arid,
            Climate::Tropical,
            Climate::Volcanic,
        ] {
            let map = generate_map(&GeneratorParams {
                climate,
                ..small(5)
            })
            .unwrap();
            for obj in &layer(&map, LAYER_FEATURES).objects {
                let MapGeometry::Stamp { icon, .. } = &obj.geometry else {
                    panic!("features must be stamps");
                };
                assert!(is_known_stamp(icon), "{icon}");
            }
        }
    }

    #[test]
    fn continent_land_share_and_one_dominant_mass() {
        for seed in [1, 42, 1234] {
            let map = generate_map(&small(seed)).unwrap();
            let actual = map.stats.land_percent_actual;
            assert!((actual - 40.0).abs() <= 15.0, "land {actual}");
            let sizes = coast::land_components(&map.grid);
            let total: usize = sizes.iter().sum();
            assert!(
                sizes[0] as f64 >= 0.6 * total as f64,
                "seed {seed}: {sizes:?}"
            );
        }
    }

    #[test]
    fn archipelago_has_many_islands() {
        for seed in [1, 42, 1234] {
            let map = generate_map(&GeneratorParams {
                land_shape: LandShape::Archipelago,
                ..small(seed)
            })
            .unwrap();
            assert!(
                map.stats.land_masses >= 8,
                "seed {seed}: {}",
                map.stats.land_masses
            );
        }
    }

    #[test]
    fn pangaea_is_one_or_two_masses() {
        for seed in [1, 42, 1234] {
            let map = generate_map(&GeneratorParams {
                land_shape: LandShape::Pangaea,
                land_percent: 60,
                ..small(seed)
            })
            .unwrap();
            assert!(
                map.stats.land_masses <= 2,
                "seed {seed}: {}",
                map.stats.land_masses
            );
        }
    }

    #[test]
    fn twin_continents_have_two_or_more_big_masses() {
        for seed in [1, 42, 1234] {
            let map = generate_map(&GeneratorParams {
                land_shape: LandShape::TwinContinents,
                ..small(seed)
            })
            .unwrap();
            let sizes = coast::land_components(&map.grid);
            let total: usize = sizes.iter().sum();
            let big = sizes
                .iter()
                .filter(|s| **s as f64 >= 0.15 * total as f64)
                .count();
            assert!(big >= 2, "seed {seed}: {sizes:?}");
        }
    }

    /// Share of land cells whose biome passes `test`.
    fn land_share(map: &GeneratedMap, test: fn(Biome) -> bool) -> f64 {
        let land: Vec<Biome> = map
            .grid
            .biome
            .iter()
            .copied()
            .filter(|b| !b.is_water())
            .collect();
        land.iter().filter(|b| test(**b)).count() as f64 / land.len() as f64
    }

    #[test]
    fn frozen_is_mostly_cold_and_arid_mostly_dry() {
        for seed in [1, 42, 1234] {
            let frozen = generate_map(&GeneratorParams {
                climate: Climate::Frozen,
                ..small(seed)
            })
            .unwrap();
            let cold = land_share(&frozen, Biome::is_cold);
            assert!(cold > 0.5, "seed {seed}: cold {cold}");
            let arid = generate_map(&GeneratorParams {
                climate: Climate::Arid,
                ..small(seed)
            })
            .unwrap();
            let dry = land_share(&arid, Biome::is_dry);
            assert!(dry > 0.5, "seed {seed}: dry {dry}");
            let temperate = generate_map(&small(seed)).unwrap();
            assert!(land_share(&temperate, Biome::is_cold) < 0.3);
        }
    }

    #[test]
    fn climate_changes_biomes_but_not_the_coastline() {
        let temperate = generate_map(&small(42)).unwrap();
        let frozen = generate_map(&GeneratorParams {
            climate: Climate::Frozen,
            temperature: -0.3,
            rainfall: 0.4,
            ..small(42)
        })
        .unwrap();
        let rings = |m: &GeneratedMap| -> Vec<MapGeometry> {
            layer(m, LAYER_TERRAIN)
                .objects
                .iter()
                .map(|o| o.geometry.clone())
                .collect()
        };
        assert_eq!(rings(&temperate), rings(&frozen));
        assert_ne!(temperate.grid.biome, frozen.grid.biome);
        assert_ne!(temperate.background_png, frozen.background_png);
    }

    #[test]
    fn every_shape_and_climate_generates() {
        for shape in [
            LandShape::Continent,
            LandShape::TwinContinents,
            LandShape::Archipelago,
            LandShape::Pangaea,
            LandShape::Coastline,
        ] {
            for land_percent in [10, 90] {
                let map = generate_map(&GeneratorParams {
                    land_shape: shape,
                    land_percent,
                    climate: Climate::Volcanic,
                    width: 256,
                    height: 512,
                    ..small(11)
                })
                .unwrap();
                assert!(map.stats.land_percent_actual > 0.0);
            }
        }
    }

    #[test]
    fn invalid_params_are_rejected() {
        let bad = GeneratorParams {
            land_percent: 95,
            ..small(1)
        };
        assert!(matches!(generate_map(&bad), Err(AppError::Validation(_))));
        assert!(matches!(
            render_preview(&bad, 256),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            render_preview(&small(1), 10),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn preview_is_a_small_png() {
        let params = GeneratorParams::default(); // 4000 × 3000
        let png = render_preview(&params, 400).unwrap();
        let img = image::load_from_memory(&png).unwrap();
        assert_eq!((img.width(), img.height()), (400, 300));
        let tall = GeneratorParams {
            width: 600,
            height: 1200,
            ..small(3)
        };
        let img = image::load_from_memory(&render_preview(&tall, 256).unwrap()).unwrap();
        assert_eq!((img.width(), img.height()), (128, 256));
    }

    fn with_places(params: GeneratorParams) -> GeneratorParams {
        GeneratorParams {
            scope: GenerationScope::Places,
            ..params
        }
    }

    fn places(map: &GeneratedMap) -> &GeneratedPlaces {
        map.places.as_ref().expect("places scope")
    }

    /// Union-find root (used to check road connectivity).
    fn root(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }

    /// Joins every pair of consecutive road nodes; returns the node sets.
    fn road_network(map: &GeneratedMap) -> Vec<usize> {
        let mut parent: Vec<usize> = (0..map.grid.layout.len()).collect();
        for road in &places(map).roads {
            for w in road.nodes.windows(2) {
                let (a, b) = (root(&mut parent, w[0]), root(&mut parent, w[1]));
                parent[a] = b;
            }
        }
        parent
    }

    /// A few varied worlds with places, at the small test size.
    fn place_worlds() -> Vec<GeneratedMap> {
        let mut out = Vec::new();
        for seed in [1, 42, 1234] {
            out.push(generate_map(&with_places(small(seed))).unwrap());
        }
        for (shape, climate) in [
            (LandShape::Archipelago, Climate::Temperate),
            (LandShape::Continent, Climate::Frozen),
            (LandShape::Continent, Climate::Arid),
            (LandShape::Pangaea, Climate::Tropical),
        ] {
            out.push(
                generate_map(&with_places(GeneratorParams {
                    land_shape: shape,
                    climate,
                    ..small(7)
                }))
                .unwrap(),
            );
        }
        out
    }

    #[test]
    fn map_only_has_no_places() {
        let map = generate_map(&small(42)).unwrap();
        assert!(map.places.is_none());
        assert_eq!((map.stats.settlements, map.stats.roads), (0, 0));
    }

    #[test]
    fn history_scope_reports_placing_then_history() {
        let params = GeneratorParams {
            scope: GenerationScope::History,
            ..small(1)
        };
        let mut phases = Vec::new();
        let mut record = |p: GeneratePhase, d: u32, t: u32| phases.push((p, d, t));
        let map = generate_map_with_progress(&params, &mut record).unwrap();
        assert_eq!(
            phases,
            [
                (GeneratePhase::Generating, 0, 1),
                (GeneratePhase::Generating, 1, 1),
                (GeneratePhase::Placing, 0, 1),
                (GeneratePhase::Placing, 1, 1),
                (GeneratePhase::History, 0, 1),
                (GeneratePhase::History, 1, 1),
            ]
        );
        let names: Vec<&str> = map.layers.iter().map(|l| l.name.as_str()).collect();
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
        let h = map.history.as_ref().unwrap();
        assert_eq!(map.stats.kingdoms as usize, h.kingdoms.len());
        assert_eq!(map.stats.characters as usize, h.characters.len());
        assert_eq!(map.stats.events as usize, h.events.len());
        assert_eq!(map.stats.documents as usize, h.documents.len());
        assert!(map.stats.kingdoms > 0);
        // The places scope has no history.
        let places_only = generate_map(&with_places(small(1))).unwrap();
        assert!(places_only.history.is_none());
        assert_eq!(places_only.stats.kingdoms, 0);
    }

    #[test]
    fn places_scope_reports_its_phase() {
        let mut phases = Vec::new();
        let mut record = |p: GeneratePhase, d: u32, t: u32| phases.push((p, d, t));
        generate_map_with_progress(&with_places(small(3)), &mut record).unwrap();
        assert_eq!(
            phases,
            [
                (GeneratePhase::Generating, 0, 1),
                (GeneratePhase::Generating, 1, 1),
                (GeneratePhase::Placing, 0, 1),
                (GeneratePhase::Placing, 1, 1),
            ]
        );
    }

    #[test]
    fn places_are_deterministic_and_vary_by_seed() {
        let a = generate_map(&with_places(small(42))).unwrap();
        let b = generate_map(&with_places(small(42))).unwrap();
        assert_eq!(a.places, b.places);
        assert_eq!(a.layers, b.layers);
        assert_eq!(a.stats, b.stats);
        let c = generate_map(&with_places(small(43))).unwrap();
        assert_ne!(places(&a).settlements, places(&c).settlements);
        let names = |m: &GeneratedMap| -> Vec<String> {
            places(m)
                .settlements
                .iter()
                .map(|s| s.name.clone())
                .collect()
        };
        assert_ne!(names(&a), names(&c));
        // The terrain is the same as without places (only the stamps under
        // settlements are cleared).
        let plain = generate_map(&small(42)).unwrap();
        assert_eq!(plain.background_png, a.background_png);
        assert_eq!(layer(&plain, LAYER_TERRAIN), layer(&a, LAYER_TERRAIN));
        assert!(
            layer(&a, LAYER_FEATURES).objects.len() <= layer(&plain, LAYER_FEATURES).objects.len()
        );
    }

    #[test]
    fn places_layers_and_stats_match() {
        let map = generate_map(&with_places(small(42))).unwrap();
        let names: Vec<&str> = map.layers.iter().map(|l| l.name.as_str()).collect();
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
        let p = places(&map);
        assert_eq!(map.stats.settlements as usize, p.settlements.len());
        assert_eq!(map.stats.roads as usize, p.roads.len());
        assert_eq!(layer(&map, LAYER_ROADS).objects.len(), p.roads.len());
        assert_eq!(
            map.stats.stamps as usize,
            layer(&map, LAYER_FEATURES).objects.len()
        );
        let objects = &layer(&map, LAYER_PLACES).objects;
        assert_eq!(objects.len(), 2 * p.settlements.len());
        for (index, s) in p.settlements.iter().enumerate() {
            let (stamp, pin) = (&objects[2 * index], &objects[2 * index + 1]);
            assert_eq!(stamp.role, ObjectRole::Settlement { index });
            assert_eq!(pin.role, ObjectRole::SettlementPin { index });
            let MapGeometry::Stamp { x, y, icon, .. } = &stamp.geometry else {
                panic!("settlement stamp expected");
            };
            assert!(is_known_stamp(icon), "{icon}");
            assert_eq!((*x, *y), (s.x, s.y));
            assert_eq!(pin.geometry, MapGeometry::Pin { x: s.x, y: s.y });
            assert_eq!(s.kind, s.tier.kind());
        }
        for road in &layer(&map, LAYER_ROADS).objects {
            assert!(road.style["dash"].as_array().is_some_and(|d| d.len() == 2));
        }
    }

    #[test]
    fn capitals_match_the_kingdom_count() {
        // Default (5 kingdoms) and the extremes. Capitals are spaced at
        // least 6 % of the map diagonal apart; only a world with very little
        // habitable land could end up with fewer (none of these do).
        for kingdoms in [1, 5, 12] {
            for seed in [1, 42] {
                let map = generate_map(&with_places(GeneratorParams {
                    kingdoms,
                    width: 1600,
                    height: 1200,
                    ..small(seed)
                }))
                .unwrap();
                let capitals = places(&map)
                    .settlements
                    .iter()
                    .filter(|s| s.tier == Tier::Capital)
                    .count();
                assert_eq!(capitals, usize::from(kingdoms), "seed {seed}");
            }
        }
    }

    #[test]
    fn settlements_are_on_habitable_land_and_spaced_out() {
        for map in place_worlds() {
            let p = places(&map);
            let grid = &map.grid;
            assert!(!p.settlements.is_empty());
            assert!(p.settlements.len() <= places::MAX_SETTLEMENTS);
            let diag = grid.layout.width.hypot(grid.layout.height);
            assert!(p.spacing.capital >= 0.06 * diag - 1e-9);
            let water = fields::distance_to_water(grid);
            for s in &p.settlements {
                let i = s.cell;
                assert!(grid.is_land(i));
                assert_eq!(grid.nearest(s.x, s.y), i);
                assert!(
                    !places::is_uninhabitable(grid.biome[i]),
                    "{:?}",
                    grid.biome[i]
                );
                assert!(!matches!(
                    grid.biome[i],
                    Biome::IceSheet | Biome::SnowyPeaks
                ));
                if grid.biome[i] == Biome::Desert {
                    // Desert settlements need a river or a coast nearby.
                    let river_near = grid.river[i]
                        || grid
                            .neighbours8(i)
                            .any(|n| grid.river[n] || grid.neighbours8(n).any(|m| grid.river[m]));
                    assert!(river_near || water[i] <= 2);
                }
            }
            for (a_i, a) in p.settlements.iter().enumerate() {
                for b in &p.settlements[a_i + 1..] {
                    let d = (a.x - b.x).hypot(a.y - b.y);
                    let min = p.spacing.for_tier(a.tier.max(b.tier));
                    assert!(d >= min - 1e-6, "{} {} {d} < {min}", a.name, b.name);
                }
            }
        }
    }

    #[test]
    fn ports_are_only_on_unfrozen_sea_coasts() {
        for map in place_worlds() {
            let grid = &map.grid;
            for s in &places(&map).settlements {
                let i = s.cell;
                let ring: Vec<usize> = grid
                    .neighbours8(i)
                    .flat_map(|n| grid.neighbours8(n).collect::<Vec<_>>())
                    .collect();
                let open_sea = ring.iter().any(|&n| grid.biome[n] == Biome::Ocean);
                if s.port {
                    assert!(open_sea, "{} is a port without open sea", s.name);
                    assert_ne!(s.tier, Tier::Village);
                } else if s.tier != Tier::Village {
                    assert!(!open_sea, "{} should be a port", s.name);
                }
            }
        }
    }

    #[test]
    fn roads_stay_on_land_and_connect_their_settlements() {
        for map in place_worlds() {
            let p = places(&map);
            let grid = &map.grid;
            let mut parent = road_network(&map);
            for road in &p.roads {
                assert!(road.nodes.len() >= 2);
                assert!(road.nodes.iter().all(|&n| grid.is_land(n)));
                assert!((2..=MAX_POINTS).contains(&road.points.len()));
                for w in road.points.windows(2) {
                    assert!(roads::segment_on_land(grid, w[0], w[1]), "road over water");
                }
            }
            for route in &p.routes {
                let a = root(&mut parent, p.settlements[route.from].cell);
                let b = root(&mut parent, p.settlements[route.to].cell);
                assert_eq!(a, b, "route {route:?} is not connected");
            }
        }
    }

    #[test]
    fn every_settlement_reaches_a_capital_on_a_continent() {
        for seed in [1, 42, 1234] {
            let map = generate_map(&with_places(small(seed))).unwrap();
            let p = places(&map);
            let mut parent = road_network(&map);
            let capitals: Vec<usize> = p
                .settlements
                .iter()
                .filter(|s| s.tier == Tier::Capital)
                .map(|s| s.cell)
                .collect();
            for s in &p.settlements {
                let mine = root(&mut parent, s.cell);
                let reached = capitals.iter().any(|&c| root(&mut parent, c) == mine);
                assert!(reached, "seed {seed}: {} is cut off", s.name);
            }
        }
    }

    #[test]
    fn settlements_and_regions_have_unique_names() {
        for map in place_worlds() {
            let p = places(&map);
            let mut seen = std::collections::HashSet::new();
            for name in p
                .settlements
                .iter()
                .map(|s| &s.name)
                .chain(p.regions.iter().map(|r| &r.name))
            {
                assert!(!name.trim().is_empty());
                assert!(seen.insert(name.to_lowercase()), "duplicate {name}");
            }
            for s in &p.settlements {
                let region = p.regions.iter().find(|r| r.mass_index == s.mass).unwrap();
                assert_eq!(s.region, region.name);
                assert!(s.summary.contains(&region.phrase), "{}", s.summary);
            }
        }
    }

    /// Writes sample backgrounds for eyeballing. Run with
    /// `cargo test dump_samples -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn dump_samples() {
        let dir = std::env::temp_dir().join("orbis-gen");
        std::fs::create_dir_all(&dir).unwrap();
        let shapes = [
            LandShape::Continent,
            LandShape::TwinContinents,
            LandShape::Archipelago,
            LandShape::Pangaea,
            LandShape::Coastline,
        ];
        let climates = [
            Climate::Temperate,
            Climate::Frozen,
            Climate::Arid,
            Climate::Tropical,
            Climate::Volcanic,
        ];
        for seed in [1, 42, 1234] {
            for shape in shapes {
                for climate in climates {
                    let map = generate_map(&GeneratorParams {
                        land_shape: shape,
                        climate,
                        ..small(seed)
                    })
                    .unwrap();
                    let name = format!("{seed}-{shape:?}-{climate:?}.png");
                    std::fs::write(dir.join(&name), &map.background_png).unwrap();
                    println!("{name}: {:?}", map.stats);
                }
            }
        }
        println!("written to {}", dir.display());
    }

    /// Places at the acceptance size: counts and timing. Run with
    /// `cargo test places_4000 -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn places_4000_by_3000_report() {
        for (shape, climate) in [
            (LandShape::Continent, Climate::Temperate),
            (LandShape::Archipelago, Climate::Temperate),
            (LandShape::Continent, Climate::Frozen),
            (LandShape::Continent, Climate::Arid),
        ] {
            let params = GeneratorParams {
                scope: GenerationScope::Places,
                land_shape: shape,
                climate,
                ..GeneratorParams::default()
            };
            let start = std::time::Instant::now();
            let map = generate_map(&params).unwrap();
            let total = start.elapsed();
            let base = generate_map(&GeneratorParams {
                scope: GenerationScope::MapOnly,
                ..params.clone()
            })
            .unwrap();
            let _ = base;
            let places = map.places.as_ref().unwrap();
            let tiers = |t: Tier| places.settlements.iter().filter(|s| s.tier == t).count();
            let ports = places.settlements.iter().filter(|s| s.port).count();
            let road_points: usize = places.roads.iter().map(|r| r.points.len()).sum();
            println!(
                "{shape:?}/{climate:?}: total {total:?}; capitals {} towns {} villages {} ports {ports}; roads {} ({road_points} pts); routes {}; spacing {:?}",
                tiers(Tier::Capital),
                tiers(Tier::Town),
                tiers(Tier::Village),
                places.roads.len(),
                places.routes.len(),
                places.spacing,
            );
            for l in &map.layers {
                println!("  layer {}: {} objects", l.name, l.objects.len());
            }
            // Overlay roads and settlements on the background for eyeballing.
            let mut img = image::load_from_memory(&map.background_png)
                .unwrap()
                .to_rgb8();
            let sx = f64::from(img.width()) / 4000.0;
            let dot = |img: &mut image::RgbImage, x: f64, y: f64, r: i64, c: [u8; 3]| {
                for dy in -r..=r {
                    for dx in -r..=r {
                        let (px, py) = ((x * sx) as i64 + dx, (y * sx) as i64 + dy);
                        if px >= 0
                            && py >= 0
                            && (px as u32) < img.width()
                            && (py as u32) < img.height()
                        {
                            img.put_pixel(px as u32, py as u32, image::Rgb(c));
                        }
                    }
                }
            };
            for road in &places.roads {
                for w in road.points.windows(2) {
                    for k in 0..=100 {
                        let t = f64::from(k) / 100.0;
                        let (x, y) = (
                            w[0].x + (w[1].x - w[0].x) * t,
                            w[0].y + (w[1].y - w[0].y) * t,
                        );
                        dot(&mut img, x, y, 2, [120, 60, 20]);
                    }
                }
            }
            for s in &places.settlements {
                let (r, c) = match s.tier {
                    Tier::Capital => (14, [200, 0, 0]),
                    Tier::Town => (9, [0, 0, 0]),
                    Tier::Village => (5, [255, 255, 255]),
                };
                dot(&mut img, s.x, s.y, r, c);
            }
            let dir = std::env::temp_dir().join("orbis-gen");
            std::fs::create_dir_all(&dir).unwrap();
            let img = image::imageops::thumbnail(&img, 1400, 1050);
            img.save(dir.join(format!("places-{shape:?}-{climate:?}.png")))
                .unwrap();
            for r in &places.regions {
                println!("  region {} ({})", r.name, r.phrase);
            }
            for s in places.settlements.iter().take(12) {
                println!("  {:?} {} — {}", s.tier, s.name, s.summary);
            }
        }
        let params = GeneratorParams {
            scope: GenerationScope::Places,
            ..GeneratorParams::default()
        };
        let layout = GridLayout::for_map(4000, 3000);
        let mut grid = fields::build_grid(&params, layout);
        rivers::trace_rivers(&mut grid, &mut rng::stage_rng(42, rng::Stage::Rivers));
        let t = std::time::Instant::now();
        let (places, _) = generate_places(&grid, &params);
        println!(
            "places stage alone {:?} ({} settlements)",
            t.elapsed(),
            places.settlements.len()
        );
    }

    /// Timing check for the acceptance size. Run with
    /// `cargo test --release generate_4000 -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn generate_4000_by_3000_timing() {
        let params = GeneratorParams::default();
        let t = std::time::Instant::now();
        let mut grid = fields::build_grid(&params, GridLayout::for_map(4000, 3000));
        println!("fields {:?}", t.elapsed());
        let t = std::time::Instant::now();
        rivers::trace_rivers(&mut grid, &mut rng::stage_rng(42, rng::Stage::Rivers));
        println!("rivers {:?}", t.elapsed());
        let t = std::time::Instant::now();
        coast::coast_polygons(&grid);
        println!("coast {:?}", t.elapsed());
        let t = std::time::Instant::now();
        scatter::scatter_stamps(
            &grid,
            params.climate,
            &mut rng::stage_rng(42, rng::Stage::Scatter),
        );
        println!("scatter {:?}", t.elapsed());
        let t = std::time::Instant::now();
        let img = render::render_image(&grid, params.climate, 4000, 3000);
        println!("render {:?}", t.elapsed());
        let t = std::time::Instant::now();
        render::encode_png(&img).unwrap();
        println!("encode {:?}", t.elapsed());

        let start = std::time::Instant::now();
        let map = generate_map(&GeneratorParams::default()).unwrap();
        println!(
            "4000x3000: {:?}, png {} KB, stats {:?}",
            start.elapsed(),
            map.background_png.len() / 1024,
            map.stats
        );
        let dir = std::env::temp_dir().join("orbis-gen");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("seed42.png"), &map.background_png).unwrap();
    }
}
