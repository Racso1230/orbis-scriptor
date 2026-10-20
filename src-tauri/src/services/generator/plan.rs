//! The generator's output: an in-memory plan the writer saves through the
//! existing services. Nothing here touches the database or the disk.

use serde_json::{Value, json};

use super::climate::{biome_color, hex, palette};
use super::coast::CoastPolygon;
use super::fields::TerrainGrid;
use super::places::{RegionName, Settlement, Spacing, Tier, stamp_size};
use super::rivers::River;
use super::roads::{Road, Route};
use super::scatter::StampPlacement;
use crate::models::generator::{Biome, Climate, GenerationStats, GeneratorParams};
use crate::models::map::MapGeometry;
use crate::services::maps::MAX_OPS;

/// Bumped whenever the same seed + params would give a different world, so
/// stored worlds can say which generator made them.
pub const GENERATOR_VERSION: u32 = 1;

pub const LAYER_TERRAIN: &str = "Terrain";
pub const LAYER_RIVERS: &str = "Rivers";
pub const LAYER_FEATURES: &str = "Features";
pub const LAYER_ROADS: &str = "Roads";
pub const LAYER_PLACES: &str = "Places";

/// What a generated object represents (for the writer, tags and Phase 14b).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectRole {
    /// Outline of a land mass (`area`).
    Coast,
    /// Outline of a lake inside land (`area`, drawn above the land).
    Lake,
    /// A river (`path`); `order` 1 (stream) ..= 4 (great river).
    River { order: u8 },
    /// A biome stamp (`stamp`).
    Feature { biome: Biome },
    /// A road (`path`), styled by the tier it was built for.
    Road { tier: Tier },
    /// The stamp of settlement `index` (into `GeneratedPlaces::settlements`).
    Settlement { index: usize },
    /// The pin of settlement `index`; the writer links it to the location
    /// entity it creates for that settlement.
    SettlementPin { index: usize },
}

/// One map object to create. Geometry and style are already valid for
/// `maps::apply_ops` (point counts, style size, stamp keys).
#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedObject {
    pub geometry: MapGeometry,
    /// JSON object, at most `MAX_STYLE_BYTES` when serialised.
    pub style: Value,
    pub role: ObjectRole,
}

/// A layer and its objects, in creation (= drawing) order.
#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedLayer {
    pub name: String,
    pub objects: Vec<GeneratedObject>,
}

impl GeneratedLayer {
    /// The objects split into groups of at most `MAX_OPS`, one group per
    /// `apply_ops` call. (`chunks` borrows the vector; no copying.)
    pub fn batches(&self) -> std::slice::Chunks<'_, GeneratedObject> {
        self.objects.chunks(MAX_OPS)
    }
}

/// The complete terrain plan for one map.
#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedMap {
    pub params: GeneratorParams,
    pub generator_version: u32,
    /// Map size in pixels (what `set_size` should end up with).
    pub width: u32,
    pub height: u32,
    /// PNG bytes of the background image.
    pub background_png: Vec<u8>,
    /// Size of that image. Equal to the map size unless the map is larger
    /// than `render::MAX_BACKGROUND_PIXELS`; then the image is smaller and
    /// the map view stretches it to the map size.
    pub background_width: u32,
    pub background_height: u32,
    /// Terrain, Rivers, Features — bottom to top.
    pub layers: Vec<GeneratedLayer>,
    pub stats: GenerationStats,
    /// The grids, kept so later stages (settlements, roads, kingdoms) can
    /// reuse height, moisture, temperature, biome and river data.
    pub grid: TerrainGrid,
    /// Rivers with their grid nodes (same order as the Rivers layer).
    pub rivers: Vec<River>,
    /// Settlements, roads and region names; `None` for the map-only scope.
    pub places: Option<GeneratedPlaces>,
}

/// The "places" part of a plan (Phase 14b).
#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedPlaces {
    /// Capitals first, then towns, then villages.
    pub settlements: Vec<Settlement>,
    /// Same order as the Roads layer.
    pub roads: Vec<Road>,
    /// The connections the roads were built for.
    pub routes: Vec<Route>,
    pub regions: Vec<RegionName>,
    /// The spacing each tier kept.
    pub spacing: Spacing,
}

/// Width of lines that should look the same on small and large maps.
fn line_scale(grid: &TerrainGrid) -> f64 {
    (grid.layout.width.max(grid.layout.height) / 2000.0).clamp(1.0, 6.0)
}

pub fn terrain_layer(
    grid: &TerrainGrid,
    climate: Climate,
    polygons: Vec<CoastPolygon>,
) -> GeneratedLayer {
    let pal = palette(climate);
    let land_fill = hex(biome_color(climate, Biome::Grassland));
    let stroke = hex(pal.coast_line);
    let width = (1.5 * line_scale(grid) * 10.0).round() / 10.0;
    let objects = polygons
        .into_iter()
        .map(|p| {
            let (fill, role) = if p.is_land {
                (land_fill.clone(), ObjectRole::Coast)
            } else {
                (hex(pal.lake), ObjectRole::Lake)
            };
            GeneratedObject {
                geometry: MapGeometry::Area { points: p.ring },
                // Nearly transparent: the background image carries the
                // detail; the area exists so the land can be selected.
                style: json!({
                    "stroke": stroke,
                    "strokeWidth": width,
                    "fill": fill,
                    "fillOpacity": 0.05,
                    "fillPattern": "none",
                    "smoothing": 0.3,
                }),
                role,
            }
        })
        .collect();
    GeneratedLayer {
        name: LAYER_TERRAIN.into(),
        objects,
    }
}

pub fn river_layer(grid: &TerrainGrid, climate: Climate, rivers: &[River]) -> GeneratedLayer {
    let stroke = hex(palette(climate).river);
    let scale = line_scale(grid);
    let objects = rivers
        .iter()
        .map(|r| {
            let width = (f64::from(r.order) * 1.2 + 0.8) * scale;
            GeneratedObject {
                geometry: MapGeometry::Path {
                    points: r.points.clone(),
                    closed: false,
                },
                style: json!({
                    "stroke": stroke,
                    "strokeWidth": (width * 10.0).round() / 10.0,
                    "smoothing": 0.5,
                }),
                role: ObjectRole::River { order: r.order },
            }
        })
        .collect();
    GeneratedLayer {
        name: LAYER_RIVERS.into(),
        objects,
    }
}

pub fn feature_layer(stamps: &[StampPlacement]) -> GeneratedLayer {
    let objects = stamps
        .iter()
        .map(|s| GeneratedObject {
            geometry: MapGeometry::Stamp {
                x: s.x,
                y: s.y,
                icon: s.icon.to_string(),
                scale: s.scale,
                rotation: s.rotation,
            },
            style: json!({}),
            role: ObjectRole::Feature { biome: s.biome },
        })
        .collect();
    GeneratedLayer {
        name: LAYER_FEATURES.into(),
        objects,
    }
}

pub fn road_layer(grid: &TerrainGrid, roads: &[Road]) -> GeneratedLayer {
    let scale = line_scale(grid);
    let objects = roads
        .iter()
        .map(|road| {
            let (width, stroke) = match road.tier {
                Tier::Capital => (3.0, "#7c4a1e"),
                Tier::Town => (2.2, "#8b5a2b"),
                Tier::Village => (1.5, "#a0784f"),
            };
            let width = (width * scale * 10.0).round() / 10.0;
            GeneratedObject {
                geometry: MapGeometry::Path {
                    points: road.points.clone(),
                    closed: false,
                },
                style: json!({
                    "stroke": stroke,
                    "strokeWidth": width,
                    "dash": [(width * 4.0).round(), (width * 2.5).round()],
                    "smoothing": 0.2,
                }),
                role: ObjectRole::Road { tier: road.tier },
            }
        })
        .collect();
    GeneratedLayer {
        name: LAYER_ROADS.into(),
        objects,
    }
}

/// Stamp key of a settlement.
pub fn settlement_icon(s: &Settlement) -> &'static str {
    match (s.tier, s.port) {
        (Tier::Village, _) => "village",
        (_, true) => "port",
        (Tier::Capital, false) => "city",
        (Tier::Town, false) => "town",
    }
}

/// One stamp and one pin per settlement. The pin comes right after its
/// stamp so it draws on top.
pub fn places_layer(grid: &TerrainGrid, settlements: &[Settlement]) -> GeneratedLayer {
    let size = stamp_size(grid);
    let mut objects = Vec::with_capacity(settlements.len() * 2);
    for (index, s) in settlements.iter().enumerate() {
        let scale = (size * s.tier.stamp_factor() * 100.0).round() / 100.0;
        objects.push(GeneratedObject {
            geometry: MapGeometry::Stamp {
                x: s.x,
                y: s.y,
                icon: settlement_icon(s).to_owned(),
                scale,
                rotation: 0.0,
            },
            style: json!({}),
            role: ObjectRole::Settlement { index },
        });
        objects.push(GeneratedObject {
            geometry: MapGeometry::Pin { x: s.x, y: s.y },
            style: json!({}),
            role: ObjectRole::SettlementPin { index },
        });
    }
    GeneratedLayer {
        name: LAYER_PLACES.into(),
        objects,
    }
}
