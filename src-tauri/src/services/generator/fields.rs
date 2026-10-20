//! The terrain grid: height, moisture, temperature and biome per grid node.
//!
//! The map is sampled on a regular grid of nodes `cell` map pixels apart
//! (node `(col, row)` sits at map pixel `(col × cell, row × cell)`). Fine
//! detail between nodes is interpolated by the renderer.

use std::collections::VecDeque;

use noise::{NoiseFn, Perlin};
use rand::RngExt;

use super::biomes::classify;
use super::climate::{SEA_ICE_BELOW, profile};
use super::coast::land_labels;
use super::land_shape::{ShapeMask, smoothstep};
use super::rng::{Stage, stage_rng};
use crate::models::generator::{Biome, Climate, GeneratorParams, LandShape};

/// Default spacing between grid nodes in map pixels.
const BASE_CELL: u32 = 8;
/// The grid is kept at or below this many nodes per side (so a 16k map is
/// still quick): 600 × 450 nodes at most.
const MAX_COLS: u32 = 600;
const MAX_ROWS: u32 = 450;

/// Grid size and spacing for a map.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridLayout {
    pub cols: usize,
    pub rows: usize,
    /// Map pixels between neighbouring nodes.
    pub cell: f64,
    /// Map size in pixels.
    pub width: f64,
    pub height: f64,
}

impl GridLayout {
    /// The full-quality layout for a map.
    pub fn for_map(width: u32, height: u32) -> GridLayout {
        let cell = BASE_CELL
            .max(width.div_ceil(MAX_COLS))
            .max(height.div_ceil(MAX_ROWS));
        GridLayout::with_cell(width, height, cell)
    }

    /// A layout with a given node spacing (the preview uses a coarser one).
    pub fn with_cell(width: u32, height: u32, cell: u32) -> GridLayout {
        let cell = cell.max(1);
        GridLayout {
            // One more node than cells, so the last node reaches the far edge.
            cols: width.div_ceil(cell) as usize + 1,
            rows: height.div_ceil(cell) as usize + 1,
            cell: f64::from(cell),
            width: f64::from(width),
            height: f64::from(height),
        }
    }

    pub fn len(&self) -> usize {
        self.cols * self.rows
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Everything the later stages (rivers, coast, scatter, rendering, and in
/// Phase 14b settlements) need to know about the terrain. All `Vec`s have
/// `cols × rows` entries in row-major order (index = `row × cols + col`).
#[derive(Debug, Clone, PartialEq)]
pub struct TerrainGrid {
    pub layout: GridLayout,
    /// Raw height. Land is `height > sea_level`.
    pub height: Vec<f64>,
    pub sea_level: f64,
    pub min_height: f64,
    pub max_height: f64,
    /// 0..1.
    pub moisture: Vec<f64>,
    /// Unitless; below 0 is freezing.
    pub temperature: Vec<f64>,
    pub biome: Vec<Biome>,
    /// True where a traced river runs (filled in by the river stage).
    pub river: Vec<bool>,
}

impl TerrainGrid {
    pub fn cols(&self) -> usize {
        self.layout.cols
    }

    pub fn rows(&self) -> usize {
        self.layout.rows
    }

    pub fn index(&self, col: usize, row: usize) -> usize {
        row * self.layout.cols + col
    }

    /// `(col, row)` of a node index.
    pub fn col_row(&self, i: usize) -> (usize, usize) {
        (i % self.layout.cols, i / self.layout.cols)
    }

    /// Map-pixel position of a node.
    pub fn node_position(&self, i: usize) -> (f64, f64) {
        let (c, r) = self.col_row(i);
        (c as f64 * self.layout.cell, r as f64 * self.layout.cell)
    }

    /// The node nearest to a map-pixel position (clamped to the grid).
    pub fn nearest(&self, x: f64, y: f64) -> usize {
        let c = (x / self.layout.cell)
            .round()
            .clamp(0.0, (self.cols() - 1) as f64) as usize;
        let r = (y / self.layout.cell)
            .round()
            .clamp(0.0, (self.rows() - 1) as f64) as usize;
        self.index(c, r)
    }

    pub fn is_land(&self, i: usize) -> bool {
        self.height[i] > self.sea_level
    }

    /// Height above sea level scaled to `0..1` (0 for water).
    pub fn elevation(&self, i: usize) -> f64 {
        let range = (self.max_height - self.sea_level).max(1e-9);
        ((self.height[i] - self.sea_level) / range).clamp(0.0, 1.0)
    }

    /// Depth below sea level scaled to `0..1` (0 for land).
    pub fn depth(&self, i: usize) -> f64 {
        let range = (self.sea_level - self.min_height).max(1e-9);
        ((self.sea_level - self.height[i]) / range).clamp(0.0, 1.0)
    }

    /// The up-to-8 neighbours of a node, as indices.
    pub fn neighbours8(&self, i: usize) -> impl Iterator<Item = usize> + '_ {
        // `impl Iterator` returns "some iterator" without naming its long
        // type. The `'_` says it borrows `self`, so the grid must outlive it.
        let (c, r) = self.col_row(i);
        let (c, r) = (c as isize, r as isize);
        let (cols, rows) = (self.cols() as isize, self.rows() as isize);
        (-1..=1)
            .flat_map(move |dy| (-1..=1).map(move |dx| (dx, dy)))
            .filter(|&(dx, dy)| dx != 0 || dy != 0)
            .filter_map(move |(dx, dy)| {
                let (nc, nr) = (c + dx, r + dy);
                (nc >= 0 && nr >= 0 && nc < cols && nr < rows).then(|| (nr * cols + nc) as usize)
            })
    }

    /// Share of nodes that are land, in percent.
    pub fn land_percent(&self) -> f64 {
        let land = (0..self.height.len()).filter(|&i| self.is_land(i)).count();
        100.0 * land as f64 / self.height.len().max(1) as f64
    }
}

/// Fractal Brownian motion: several octaves of noise, each twice the
/// frequency and half the strength of the last. Result is roughly `-1..1`.
fn fbm(noise: &Perlin, x: f64, y: f64, octaves: u32) -> f64 {
    let (mut sum, mut amp, mut freq, mut norm) = (0.0, 1.0, 1.0, 0.0);
    for octave in 0..octaves {
        // A per-octave offset avoids the artefact where every octave is 0
        // at the origin.
        let o = f64::from(octave) * 17.31;
        sum += amp * noise.get([x * freq + o, y * freq - o]);
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    sum / norm
}

/// The noise functions used by the field stages, seeded from the noise rng.
struct NoiseSet {
    height: Perlin,
    warp_x: Perlin,
    warp_y: Perlin,
    moisture: Perlin,
    temperature: Perlin,
    lava: Perlin,
}

impl NoiseSet {
    fn new(seed: u32) -> NoiseSet {
        let mut rng = stage_rng(seed, Stage::Noise);
        // Draw the seeds in a fixed order so each field always gets the same one.
        let mut next = || Perlin::new(rng.random_range(0..u32::MAX));
        NoiseSet {
            height: next(),
            warp_x: next(),
            warp_y: next(),
            moisture: next(),
            temperature: next(),
            lava: next(),
        }
    }
}

/// Heights only: depends on seed, size and land shape, never on climate,
/// so changing the climate keeps the same coastline.
fn height_field(params: &GeneratorParams, layout: &GridLayout, noise: &NoiseSet) -> Vec<f64> {
    let aspect = layout.width / layout.height;
    let mut shape_rng = stage_rng(params.seed, Stage::Shape);
    let mask = ShapeMask::new(params.land_shape, aspect, &mut shape_rng);
    let mut heights = Vec::with_capacity(layout.len());
    for row in 0..layout.rows {
        for col in 0..layout.cols {
            let u = (col as f64 * layout.cell / layout.width).min(1.0);
            let v = (row as f64 * layout.cell / layout.height).min(1.0);
            let (x, y) = (u * aspect, v);
            // Domain warp: shift the sample point by low-frequency noise so
            // coasts get bays and peninsulas instead of round blobs.
            let wx = fbm(&noise.warp_x, x * 2.0, y * 2.0, 3) * 0.12;
            let wy = fbm(&noise.warp_y, x * 2.0, y * 2.0, 3) * 0.12;
            let (x, y) = (x + wx, y + wy);
            let n = fbm(&noise.height, x * mask.frequency, y * mask.frequency, 6);
            let mut h = mask.value(x, y) * mask.mask_weight + n * mask.noise_weight;
            if !mask.land_on_edges() {
                // Push the border down so land rarely touches the map edge.
                let edge = u.min(1.0 - u).min(v.min(1.0 - v));
                h -= (1.0 - smoothstep(0.0, 0.04, edge)) * 0.8;
            }
            heights.push(h);
        }
    }
    heights
}

/// The height below which `1 - land_percent` of the nodes lie, so the land
/// share matches the slider.
fn sea_level_for(heights: &[f64], land_percent: u8) -> f64 {
    let mut sorted = heights.to_vec();
    // f64 is not `Ord` (because of NaN), so sort with `total_cmp`.
    sorted.sort_by(f64::total_cmp);
    let water_share = 1.0 - f64::from(land_percent) / 100.0;
    let idx = ((sorted.len() as f64 * water_share) as usize).min(sorted.len() - 1);
    sorted[idx]
}

/// Water nodes connected to the map border (4-neighbour flood fill).
fn ocean_mask(grid: &TerrainGrid) -> Vec<bool> {
    let (cols, rows) = (grid.cols(), grid.rows());
    let mut ocean = vec![false; grid.layout.len()];
    let mut queue = VecDeque::new();
    for (i, is_ocean) in ocean.iter_mut().enumerate() {
        let (c, r) = grid.col_row(i);
        let border = c == 0 || r == 0 || c == cols - 1 || r == rows - 1;
        if border && !grid.is_land(i) {
            *is_ocean = true;
            queue.push_back(i);
        }
    }
    while let Some(i) = queue.pop_front() {
        let (c, r) = grid.col_row(i);
        let mut visit = |n: usize| {
            if !ocean[n] && !grid.is_land(n) {
                ocean[n] = true;
                queue.push_back(n);
            }
        };
        if c > 0 {
            visit(i - 1);
        }
        if c + 1 < cols {
            visit(i + 1);
        }
        if r > 0 {
            visit(i - cols);
        }
        if r + 1 < rows {
            visit(i + cols);
        }
    }
    ocean
}

/// Grid steps from every node to the nearest water node (8-neighbour BFS).
pub fn distance_to_water(grid: &TerrainGrid) -> Vec<u32> {
    let mut dist = vec![u32::MAX; grid.layout.len()];
    let mut queue = VecDeque::new();
    for (i, d) in dist.iter_mut().enumerate() {
        if !grid.is_land(i) {
            *d = 0;
            queue.push_back(i);
        }
    }
    while let Some(i) = queue.pop_front() {
        let next = dist[i] + 1;
        // Collect first: we cannot borrow `grid` for the iterator and push
        // to `queue` inside the same closure without fuss.
        let ns: Vec<usize> = grid.neighbours8(i).collect();
        for n in ns {
            if dist[n] > next {
                dist[n] = next;
                queue.push_back(n);
            }
        }
    }
    dist
}

/// Builds the full grid: heights, sea level, moisture, temperature, biomes.
/// Rivers are added later (`grid.river` starts all false).
pub fn build_grid(params: &GeneratorParams, layout: GridLayout) -> TerrainGrid {
    let noise = NoiseSet::new(params.seed);
    let height = height_field(params, &layout, &noise);
    let sea_level = sea_level_for(&height, params.land_percent);
    let min_height = height.iter().copied().fold(f64::INFINITY, f64::min);
    let max_height = height.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let n = layout.len();
    let mut grid = TerrainGrid {
        layout,
        height,
        sea_level,
        min_height,
        max_height,
        moisture: vec![0.0; n],
        temperature: vec![0.0; n],
        biome: vec![Biome::Ocean; n],
        river: vec![false; n],
    };
    if params.land_shape == LandShape::Pangaea {
        sink_small_islands(&mut grid);
    }
    apply_climate(&mut grid, params, &noise);
    grid
}

/// Pangaea is one land mass: islands much smaller than the main one are
/// lowered to just below sea level (they show as shallows in the image).
fn sink_small_islands(grid: &mut TerrainGrid) {
    let (labels, sizes) = land_labels(grid);
    let largest = sizes.iter().copied().max().unwrap_or(0);
    let below = grid.sea_level - (grid.sea_level - grid.min_height) * 0.02;
    for (i, label) in labels.iter().enumerate() {
        if *label != u32::MAX && sizes[*label as usize] * 20 < largest {
            grid.height[i] = below;
        }
    }
}

/// Moisture, temperature and biome for every node.
fn apply_climate(grid: &mut TerrainGrid, params: &GeneratorParams, noise: &NoiseSet) {
    let profile = profile(params.climate);
    let layout = grid.layout;
    let aspect = layout.width / layout.height;
    let ocean = ocean_mask(grid);
    let to_water = distance_to_water(grid);
    let span = layout.cols.max(layout.rows) as f64;
    let mountain_level = land_quantile(grid, 0.93);
    let hill_level = land_quantile(grid, 0.80);

    for i in 0..layout.len() {
        let (c, r) = grid.col_row(i);
        let u = (c as f64 * layout.cell / layout.width).min(1.0);
        let v = (r as f64 * layout.cell / layout.height).min(1.0);
        let (x, y) = (u * aspect, v);
        let elev = grid.elevation(i);

        // Moisture: noise + wetter near water + preset + slider.
        let coast = 1.0 - smoothstep(0.0, 0.12, f64::from(to_water[i]) / span);
        let m = 0.5 + 0.5 * fbm(&noise.moisture, x * 2.5, y * 2.5, 4);
        let m = m + 0.25 * coast + profile.base_moisture + params.rainfall * 0.35;
        let moisture = m.clamp(0.0, 1.0);

        // Temperature: preset − latitude − altitude + slider (+ a little noise).
        let lat = (v - 0.5).abs() * 2.0;
        let t = profile.base_temperature - profile.latitude_drop * lat * lat
            // `powf(1.5)`: lowlands barely cool, peaks cool a lot.
            - profile.lapse_rate * elev.powf(1.5)
            + params.temperature * 0.4
            + 0.12 * fbm(&noise.temperature, x * 3.0, y * 3.0, 3);

        grid.moisture[i] = moisture;
        grid.temperature[i] = t;
        grid.biome[i] = if grid.is_land(i) {
            let lava = if params.climate == Climate::Volcanic {
                fbm(&noise.lava, x * 6.0, y * 6.0, 3)
            } else {
                0.0
            };
            let h = grid.height[i];
            let relief = if h >= mountain_level {
                Relief::Mountain
            } else if h >= hill_level {
                Relief::Hills
            } else {
                Relief::Low
            };
            classify(params.climate, relief, elev, moisture, t, lava)
        } else if t < SEA_ICE_BELOW {
            Biome::SeaIce
        } else if ocean[i] {
            Biome::Ocean
        } else {
            Biome::Lake
        };
    }
}

/// Coarse landform of a land node, from height percentiles of all land.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relief {
    Low,
    Hills,
    Mountain,
}

/// The height below which `q` of the land nodes lie.
fn land_quantile(grid: &TerrainGrid, q: f64) -> f64 {
    let mut land: Vec<f64> = grid
        .height
        .iter()
        .copied()
        .filter(|h| *h > grid.sea_level)
        .collect();
    if land.is_empty() {
        return f64::INFINITY;
    }
    land.sort_by(f64::total_cmp);
    let idx = ((land.len() as f64 * q) as usize).min(land.len() - 1);
    land[idx]
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn layout_caps_grid_size() {
        let small = GridLayout::for_map(800, 600);
        assert_eq!(small.cell, 8.0);
        assert_eq!((small.cols, small.rows), (101, 76));
        let big = GridLayout::for_map(16384, 16384);
        assert!(big.cols <= MAX_COLS as usize + 1 && big.rows <= MAX_ROWS as usize + 1);
    }

    #[test]
    fn sea_level_matches_land_share() {
        let params = GeneratorParams {
            width: 800,
            height: 600,
            land_percent: 30,
            ..GeneratorParams::default()
        };
        let grid = build_grid(&params, GridLayout::for_map(800, 600));
        assert!((grid.land_percent() - 30.0).abs() < 1.0);
    }
}
