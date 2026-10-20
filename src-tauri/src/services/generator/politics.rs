//! Kingdoms (Phase 14c): grow one kingdom from every capital with a
//! weighted flood fill, find who borders whom, pick allies and enemies, and
//! outline each kingdom as map areas.
//!
//! The flood fill is Dijkstra's shortest-path search started from all
//! capitals at once: every land node goes to the capital it is "cheapest"
//! to reach. Steps cost more uphill, in mountains, swamps and deserts, and
//! when crossing a river, so ridges and rivers tend to become borders.

use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap};

use rand::RngExt;
use rand_chacha::ChaCha8Rng;
use serde_json::json;

use super::coast::{field_polygons, min_island_cells};
use super::fields::TerrainGrid;
use super::places::{Settlement, Tier};
use super::plan::{GeneratedLayer, GeneratedObject, LAYER_KINGDOMS, ObjectRole};
use crate::models::generator::Biome;
use crate::models::map::MapGeometry;

/// `owner` value of a node no kingdom claims (water, or land without a
/// capital).
pub const UNCLAIMED: u32 = u32::MAX;

/// Distinct kingdom colours (at most 12 kingdoms, see `GeneratorParams`).
const PALETTE: [&str; 12] = [
    "#c0392b", "#2874a6", "#229954", "#d68910", "#7d3c98", "#138d75", "#ba4a00", "#a93273",
    "#7d8c1c", "#1f3a93", "#b7950b", "#566573",
];

/// The political map, before names and people are added.
#[derive(Debug, Clone, PartialEq)]
pub struct Realms {
    /// Kingdom index per grid node, or [`UNCLAIMED`].
    pub owner: Vec<u32>,
    pub realms: Vec<Realm>,
}

/// One kingdom's territory and diplomacy.
#[derive(Debug, Clone, PartialEq)]
pub struct Realm {
    /// Index of its capital in the settlement list.
    pub capital: usize,
    pub cells: usize,
    /// Kingdoms sharing a land border, ascending.
    pub neighbours: Vec<usize>,
    pub allies: Vec<usize>,
    pub enemies: Vec<usize>,
    pub colour: String,
    pub stroke: String,
    /// Alternate kingdoms get a hatched fill so neighbours stand apart.
    pub hatch: bool,
}

/// The up-to-4 direct neighbours of a node (no diagonals). Kingdoms grow
/// through these only, so every kingdom is connected the same way the land
/// masses are labelled (`coast::land_labels`).
fn neighbours4(grid: &TerrainGrid, i: usize) -> impl Iterator<Item = usize> {
    let (c, r) = grid.col_row(i);
    let cols = grid.cols();
    let ns = [
        (c > 0).then(|| i - 1),
        (c + 1 < cols).then(|| i + 1),
        (r > 0).then(|| i - cols),
        (r + 1 < grid.rows()).then(|| i + cols),
    ];
    // `.flatten()` drops the `None`s.
    ns.into_iter().flatten()
}

/// How hard a biome is to march through (added to every step into it).
fn biome_cost(b: Biome) -> u32 {
    match b {
        Biome::SnowyPeaks => 500,
        Biome::Lava => 400,
        Biome::Mountain | Biome::IceSheet => 300,
        Biome::Swamp => 80,
        Biome::Hills => 60,
        Biome::Jungle => 50,
        Biome::Desert => 40,
        Biome::Taiga | Biome::Tundra => 30,
        _ => 0,
    }
}

/// Cost of one step from node `from` into node `to` (both land).
fn step_cost(grid: &TerrainGrid, from: usize, to: usize) -> u32 {
    let climb = (grid.elevation(to) - grid.elevation(from)).abs();
    let mut cost = 100 + (climb * 4000.0) as u32 + (grid.elevation(to) * 100.0) as u32;
    cost += biome_cost(grid.biome[to]);
    // Crossing onto a river: rivers make natural, but soft, borders.
    if grid.river[to] && !grid.river[from] {
        cost += 250;
    }
    cost
}

/// Grows one kingdom per capital and returns the owner of every node
/// (kingdom `k` is the `k`-th capital in `capitals`).
pub fn grow_kingdoms(grid: &TerrainGrid, capitals: &[usize]) -> Vec<u32> {
    let n = grid.layout.len();
    let mut dist = vec![u32::MAX; n];
    let mut owner = vec![UNCLAIMED; n];
    // `BinaryHeap` pops the LARGEST item; wrapping in `Reverse` flips the
    // order so the cheapest node comes out first. Ties go to the lower
    // node index, so the result never depends on hash order or timing.
    let mut heap = BinaryHeap::new();
    for (k, &cell) in capitals.iter().enumerate() {
        dist[cell] = 0;
        owner[cell] = k as u32;
        heap.push(Reverse((0u32, cell)));
    }
    while let Some(Reverse((d, i))) = heap.pop() {
        if d > dist[i] {
            continue; // an outdated entry; a cheaper path was found already
        }
        for next in neighbours4(grid, i) {
            if !grid.is_land(next) {
                continue;
            }
            let nd = d.saturating_add(step_cost(grid, i, next));
            if nd < dist[next] {
                dist[next] = nd;
                owner[next] = owner[i];
                heap.push(Reverse((nd, next)));
            }
        }
    }
    owner
}

/// Pairs `(a, b)`, `a < b`, of kingdoms whose nodes touch.
pub fn border_pairs(grid: &TerrainGrid, owner: &[u32]) -> BTreeSet<(usize, usize)> {
    let mut pairs = BTreeSet::new();
    for i in 0..owner.len() {
        if owner[i] == UNCLAIMED {
            continue;
        }
        for j in neighbours4(grid, i) {
            let (a, b) = (owner[i], owner[j]);
            if b != UNCLAIMED && a < b {
                pairs.insert((a as usize, b as usize));
            }
        }
    }
    pairs
}

/// `#rrggbb` with every channel scaled by `factor` (< 1 darkens).
fn shade(hex: &str, factor: f64) -> String {
    let channel = |at: usize| {
        let v = u8::from_str_radix(hex.get(at..at + 2).unwrap_or("80"), 16).unwrap_or(128);
        (f64::from(v) * factor).round().clamp(0.0, 255.0) as u8
    };
    format!("#{:02x}{:02x}{:02x}", channel(1), channel(3), channel(5))
}

/// Grows the kingdoms of every capital in `settlements`, then assigns
/// colours and diplomacy from `rng` (the politics stream).
pub fn build_realms(
    grid: &TerrainGrid,
    settlements: &[Settlement],
    rng: &mut ChaCha8Rng,
) -> Realms {
    let capitals: Vec<usize> = settlements
        .iter()
        .enumerate()
        .filter(|(_, s)| s.tier == Tier::Capital)
        .map(|(i, _)| i)
        .collect();
    let cells: Vec<usize> = capitals.iter().map(|&i| settlements[i].cell).collect();
    let owner = grow_kingdoms(grid, &cells);

    let mut sizes = vec![0usize; capitals.len()];
    for &o in &owner {
        if o != UNCLAIMED {
            sizes[o as usize] += 1;
        }
    }

    // A shuffled palette: neighbouring kingdoms rarely share a hue family.
    let mut colours: Vec<&str> = PALETTE.to_vec();
    for i in (1..colours.len()).rev() {
        let j = rng.random_range(0..=i);
        colours.swap(i, j);
    }

    let mut realms: Vec<Realm> = capitals
        .iter()
        .enumerate()
        .map(|(k, &capital)| {
            let colour = colours[k % colours.len()].to_owned();
            Realm {
                capital,
                cells: sizes[k],
                neighbours: Vec::new(),
                allies: Vec::new(),
                enemies: Vec::new(),
                stroke: shade(&colour, 0.6),
                colour,
                hatch: k % 2 == 1,
            }
        })
        .collect();

    // Diplomacy between neighbours: enemies, allies or neither. Every world
    // with any border gets at least one rivalry, so there are wars to tell.
    let pairs: Vec<(usize, usize)> = border_pairs(grid, &owner).into_iter().collect();
    let mut enemies = Vec::new();
    let mut allies = Vec::new();
    for &(a, b) in &pairs {
        realms[a].neighbours.push(b);
        realms[b].neighbours.push(a);
        let roll: f64 = rng.random();
        if roll < 0.45 {
            enemies.push((a, b));
        } else if roll < 0.75 {
            allies.push((a, b));
        }
    }
    if enemies.is_empty() && !pairs.is_empty() {
        let pick = pairs[rng.random_range(0..pairs.len())];
        allies.retain(|p| *p != pick);
        enemies.push(pick);
    }
    for (a, b) in enemies {
        realms[a].enemies.push(b);
        realms[b].enemies.push(a);
    }
    for (a, b) in allies {
        realms[a].allies.push(b);
        realms[b].allies.push(a);
    }
    for r in &mut realms {
        r.neighbours.sort_unstable();
        r.allies.sort_unstable();
        r.enemies.sort_unstable();
    }
    Realms { owner, realms }
}

/// Width of kingdom borders (same scaling as the coast line).
fn border_width(grid: &TerrainGrid) -> f64 {
    let scale = (grid.layout.width.max(grid.layout.height) / 2000.0).clamp(1.0, 6.0);
    (2.0 * scale * 10.0).round() / 10.0
}

/// The "Kingdoms" layer: one `area` per connected region of each kingdom.
///
/// Each kingdom is outlined with the same marching squares as the coast,
/// on a field that is positive on its own land and negative everywhere
/// else. On the coast the field is height minus sea level, so the outline
/// follows the coastline; between two kingdoms both outlines cross at the
/// same point, so their borders meet.
pub fn kingdom_layer(grid: &TerrainGrid, realms: &Realms) -> GeneratedLayer {
    let cell = grid.layout.cell;
    let min_area = min_island_cells(grid) as f64 * cell * cell;
    let width = border_width(grid);
    let mut pieces = Vec::new();
    for (k, realm) in realms.realms.iter().enumerate() {
        let values: Vec<f64> = (0..grid.layout.len())
            .map(|i| {
                let above = grid.height[i] - grid.sea_level;
                if realms.owner[i] == k as u32 {
                    above
                } else if above > 0.0 {
                    -above
                } else {
                    above
                }
            })
            .collect();
        for poly in field_polygons(grid, &values, min_area) {
            // Holes (lakes, enclaves) are left filled: only outer rings.
            if !poly.is_land {
                continue;
            }
            let style = json!({
                "stroke": realm.stroke,
                "strokeWidth": width,
                "fill": realm.colour,
                "fillOpacity": 0.18,
                "fillPattern": if realm.hatch { "hatch" } else { "none" },
                "smoothing": 0.2,
            });
            pieces.push((
                poly.area,
                GeneratedObject {
                    geometry: MapGeometry::Area { points: poly.ring },
                    style,
                    role: ObjectRole::Kingdom { index: k },
                },
            ));
        }
    }
    // Largest first, so an enclave draws on top of the kingdom around it.
    pieces.sort_by(|a, b| b.0.total_cmp(&a.0));
    GeneratedLayer {
        name: LAYER_KINGDOMS.into(),
        objects: pieces.into_iter().map(|(_, o)| o).collect(),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn shade_darkens_each_channel() {
        assert_eq!(shade("#ff8000", 0.5), "#804000");
        assert_eq!(shade("#000000", 0.6), "#000000");
    }
}
