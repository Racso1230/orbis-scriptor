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
//! 6. `plan`    — Terrain / Rivers / Features layers + stats

mod biomes;
mod climate;
pub mod coast;
pub mod fields;
mod geometry;
mod land_shape;
pub mod plan;
pub mod render;
pub mod rivers;
mod rng;
pub mod scatter;
pub mod stamps;
pub mod writer;

use std::collections::BTreeMap;

use crate::error::{AppError, AppResult};
use crate::models::generator::{GenerationStats, GeneratorParams};

pub use fields::{GridLayout, TerrainGrid};
pub use plan::{
    GENERATOR_VERSION, GeneratedLayer, GeneratedMap, GeneratedObject, LAYER_FEATURES, LAYER_RIVERS,
    LAYER_TERRAIN, ObjectRole,
};
pub use stamps::{is_known_stamp, known_stamp_keys};

/// Generates the full terrain plan for a map.
pub fn generate_map(params: &GeneratorParams) -> AppResult<GeneratedMap> {
    params.validate()?;
    let layout = GridLayout::for_map(params.width, params.height);
    let mut grid = fields::build_grid(params, layout);

    let mut river_rng = rng::stage_rng(params.seed, rng::Stage::Rivers);
    let rivers = rivers::trace_rivers(&mut grid, &mut river_rng);
    let polygons = coast::coast_polygons(&grid);
    let mut scatter_rng = rng::stage_rng(params.seed, rng::Stage::Scatter);
    let stamps = scatter::scatter_stamps(&grid, params.climate, &mut scatter_rng);

    let (bw, bh) = render::background_size(params.width, params.height);
    let image = render::render_image(&grid, params.climate, bw, bh);
    let background_png = render::encode_png(&image)?;

    let stats = stats(&grid, rivers.len(), stamps.len());
    let layers = vec![
        plan::terrain_layer(&grid, params.climate, polygons),
        plan::river_layer(&grid, params.climate, &rivers),
        plan::feature_layer(&stamps),
    ];
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
    })
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
                        other => panic!("unexpected geometry {other:?}"),
                    }
                    let in_map =
                        |x: f64, y: f64| (0.0..=800.0).contains(&x) && (0.0..=600.0).contains(&y);
                    match &obj.geometry {
                        MapGeometry::Area { points } | MapGeometry::Path { points, .. } => {
                            assert!(points.iter().all(|p| in_map(p.x, p.y)));
                        }
                        MapGeometry::Stamp { x, y, .. } => assert!(in_map(*x, *y)),
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
